use serde::{Deserialize, Serialize};

/// Key `AppSettings` is stored under in `eframe`'s persistence backend.
pub const STORAGE_KEY: &str = "donutex_settings";

/// User preferences that persist across launches, as opposed to per-session
/// state (open tabs, panel visibility, etc.) which resets every run.
#[derive(Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default = "default_theme_name")]
    pub theme_name: String,
    #[serde(default)]
    pub autosave: bool,
    #[serde(default = "default_autosave_delay_ms")]
    pub autosave_delay_ms: u64,
}

fn default_theme_name() -> String {
    "Dark+".to_string()
}

fn default_autosave_delay_ms() -> u64 {
    1000
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme_name: "Dark+".to_string(),
            autosave: false,
            autosave_delay_ms: 1000,
        }
    }
}
