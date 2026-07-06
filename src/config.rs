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
    /// Active character: "cat" | "rick". Unknown values behave as "cat".
    pub character: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            color_name: "orange".into(),
            character: "cat".into(),
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
            cfg.apply(&text);
        }
        cfg
    }

    /// Parse `key=value` lines into self. Unknown keys (incl. legacy
    /// `pattern=`) and empty values are ignored.
    fn apply(&mut self, text: &str) {
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let (k, v) = (k.trim(), v.trim());
            match k {
                "color" if !v.is_empty() => self.color_name = v.to_string(),
                "character" if !v.is_empty() => self.character = v.to_string(),
                _ => {}
            }
        }
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let body = format!("color={}\ncharacter={}\n", self.color_name, self.character);
        let _ = std::fs::write(path, body);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_character_line() {
        let mut cfg = Config::default();
        cfg.apply("color=black\ncharacter=rick\n");
        assert_eq!(cfg.color_name, "black");
        assert_eq!(cfg.character, "rick");
    }

    #[test]
    fn missing_character_defaults_to_cat() {
        let mut cfg = Config::default();
        cfg.apply("color=white\n");
        assert_eq!(cfg.character, "cat");
    }

    #[test]
    fn legacy_and_junk_lines_ignored() {
        let mut cfg = Config::default();
        cfg.apply("pattern=tabby\ngarbage\ncharacter=\n");
        assert_eq!(cfg.character, "cat"); // empty value ignored
        assert_eq!(cfg.color_name, "orange");
    }
}
