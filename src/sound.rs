//! Sound playback. We deliberately shell out to a system audio player rather than
//! linking an audio backend — it keeps the binary small, avoids ALSA/Pulse/PipeWire
//! build coupling, and reuses whatever the user already has working.
//!
//! We probe once for the first available player and cache it.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

static PLAYER: OnceLock<Option<&'static str>> = OnceLock::new();

fn player() -> Option<&'static str> {
    *PLAYER.get_or_init(|| {
        for cand in ["paplay", "pw-play", "aplay", "ffplay", "canberra-gtk-play"] {
            if which(cand) {
                return Some(cand);
            }
        }
        None
    })
}

fn which(bin: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {bin}"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Play a WAV file at `path` without blocking the caller. Fire-and-forget.
pub fn play(path: &PathBuf) {
    let Some(p) = player() else {
        return;
    };
    if !path.exists() {
        return;
    }
    let mut cmd = Command::new(p);
    match p {
        "ffplay" => {
            cmd.args(["-nodisp", "-autoexit", "-loglevel", "quiet"]);
            cmd.arg(path);
        }
        _ => {
            cmd.arg(path);
        }
    }
    let _ = cmd
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .spawn(); // detached; we never wait on it
}
