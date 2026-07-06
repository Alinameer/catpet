//! Tiny line-based IPC over a Unix domain socket.
//!
//! The running pet listens on `$XDG_RUNTIME_DIR/pixelpal.sock` (falling back to
//! `/tmp/pixelpal-$UID.sock`). Any process can connect and write a single command
//! line to poke the pet:
//!
//!   meow            -> Claude finished: jump + meow
//!   pomodoro        -> toggle the pomodoro timer
//!   stretch         -> trigger a stretch reminder now
//!   color <name>    -> set fur colour (see palette.rs names)
//!   character <name> -> set character: cat | rick
//!   pattern <name>  -> set fur pattern: solid | tabby | spots | tuxedo
//!   quit            -> exit the pet
//!
//! This is how `pixelpal meow` (the CLI subcommand) and the Claude Code Stop hook
//! talk to the already-running window.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::thread;

#[derive(Clone, Debug)]
pub enum Command {
    Meow,
    TogglePomodoro,
    Stretch,
    SetColor(String),
    SetCharacter(String),
    SetPattern(String),
    SetUserName(String),
    Quit,
}

/// Canonical socket path for this user.
pub fn socket_path() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir).join("pixelpal.sock");
        }
    }
    let uid = users_uid();
    PathBuf::from(format!("/tmp/pixelpal-{uid}.sock"))
}

fn users_uid() -> u32 {
    // Avoid pulling in a crate just for getuid.
    // SAFETY: getuid is always safe and never fails.
    unsafe { libc_getuid() }
}

extern "C" {
    #[link_name = "getuid"]
    fn libc_getuid() -> u32;
}

fn parse(line: &str) -> Option<Command> {
    let line = line.trim();
    let (cmd, rest) = match line.split_once(char::is_whitespace) {
        Some((c, r)) => (c, r.trim()),
        None => (line, ""),
    };
    let arg = if rest.is_empty() {
        None
    } else {
        Some(rest.to_string())
    };
    match cmd {
        "meow" => Some(Command::Meow),
        "pomodoro" => Some(Command::TogglePomodoro),
        "stretch" => Some(Command::Stretch),
        "color" => arg.map(Command::SetColor),
        "character" => arg.map(Command::SetCharacter),
        "pattern" => arg.map(Command::SetPattern),
        "name" => arg.map(Command::SetUserName),
        "quit" => Some(Command::Quit),
        _ => None,
    }
}

/// Start the IPC server thread. Removes any stale socket first. Commands are
/// forwarded to the main loop via `tx`, and `wake` is called so the event loop
/// can redraw promptly.
pub fn serve(tx: Sender<Command>, wake: impl Fn() + Send + 'static) {
    let path = socket_path();
    let _ = std::fs::remove_file(&path); // clear stale socket
    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[pixelpal] could not bind IPC socket {path:?}: {e}");
            return;
        }
    };

    thread::Builder::new()
        .name("pixelpal-ipc".into())
        .spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let reader = BufReader::new(stream);
                for line in reader.lines().map_while(Result::ok) {
                    if let Some(cmd) = parse(&line) {
                        let _ = tx.send(cmd);
                        wake();
                    }
                }
            }
        })
        .expect("failed to spawn ipc thread");
}

/// Client side: connect to a running pet and send one command line.
/// Returns Ok(()) if the message was written, Err if no pet is listening.
pub fn send(line: &str) -> std::io::Result<()> {
    let path = socket_path();
    let mut stream = UnixStream::connect(&path)?;
    stream.write_all(line.as_bytes())?;
    stream.write_all(b"\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_character_command() {
        assert!(matches!(
            parse("character rick"),
            Some(Command::SetCharacter(n)) if n == "rick"
        ));
        assert!(parse("character").is_none()); // arg required
    }
}
