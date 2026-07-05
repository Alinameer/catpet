//! Persisted user preferences (just fur colour now that sprites carry the art).
//! Stored as a tiny text file so the cat looks the same across restarts.
//! Hand-rolled parse/serialize to avoid a serde dependency.
//!
//! We still tolerate a legacy `pattern=` line in old config files (ignored).

use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Config {
    /// One of the sprite colours: orange | black | brown | white.
    pub color_name: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            color_name: "orange".into(),
        }
    }
}

impl Config {
    fn path() -> PathBuf {
        let base = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
                PathBuf::from(home).join(".config")
            });
        base.join("catpet").join("config.txt")
    }

    pub fn load() -> Self {
        let mut cfg = Config::default();
        if let Ok(text) = std::fs::read_to_string(Self::path()) {
            for line in text.lines() {
                let Some((k, v)) = line.split_once('=') else {
                    continue;
                };
                let (k, v) = (k.trim(), v.trim());
                if k == "color" && !v.is_empty() {
                    cfg.color_name = v.to_string();
                }
                // legacy `pattern=` lines are ignored
            }
        }
        cfg
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let body = format!("color={}\n", self.color_name);
        let _ = std::fs::write(path, body);
    }
}
