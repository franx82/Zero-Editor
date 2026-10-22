//! View preferences shared by every window and kept across launches.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_FONT_SIZE: f32 = 15.0;
pub const MIN_FONT_SIZE: f32 = 10.0;
pub const MAX_FONT_SIZE: f32 = 32.0;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub font_size: f32,
    pub word_wrap: bool,
    pub show_whitespace: bool,
    pub highlight: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            font_size: DEFAULT_FONT_SIZE,
            word_wrap: true,
            show_whitespace: false,
            highlight: true,
        }
    }
}

fn path() -> PathBuf {
    crate::session::session_dir().join("settings.json")
}

impl Prefs {
    /// Missing or unreadable settings fall back to the defaults.
    pub fn load() -> Self {
        std::fs::read(path())
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Prefs>(&bytes).ok())
            .map(|prefs| Prefs {
                font_size: prefs.font_size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE),
                ..prefs
            })
            .unwrap_or_default()
    }

    /// Written off the UI thread; a lost write only costs a preference.
    pub fn save(self) {
        std::thread::spawn(move || {
            let path = path();
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_vec_pretty(&self) {
                let _ = crate::storage::atomic_write(&path, &json);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_settings_keep_defaults_for_missing_fields() {
        let prefs: Prefs = serde_json::from_str(r#"{"word_wrap": false}"#).unwrap();
        assert!(!prefs.word_wrap);
        assert_eq!(prefs.font_size, DEFAULT_FONT_SIZE);
        assert!(prefs.highlight);
    }
}
