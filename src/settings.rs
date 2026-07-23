use serde::{Deserialize, Serialize};

/// Key `AppSettings` is stored under in `eframe`'s persistence backend.
pub const STORAGE_KEY: &str = "donutex_settings";

/// User preferences that persist across launches, as opposed to per-session
/// state (open tabs, panel visibility, etc.) which resets every run.
#[derive(Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub theme_name: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme_name: "Dark+".to_string(),
        }
    }
}
