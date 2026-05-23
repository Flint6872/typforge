use gpui::{App, Global};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppSettings {
    pub theme: String,
    pub font_size: f32,
    pub default_save_folder: Option<String>,
}

impl Global for AppSettings {}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "Tokyo Storm".into(),
            font_size: 16.0,
            default_save_folder: None,
        }
    }
}

pub fn load_settings(cx: &mut App) {
    let path = std::path::Path::new("settings.json");

    let settings = if path.exists() {
        let content = std::fs::read_to_string(path).unwrap_or_default();
        serde_json::from_str::<AppSettings>(&content).unwrap_or_else(|_| AppSettings::default())
    } else {
        let default = AppSettings::default();
        // Save the defaults to the file immediately
        if let Ok(json) = serde_json::to_string_pretty(&default) {
            let _ = std::fs::write(path, json);
        }
        default
    };

    cx.set_global(settings);
}

pub fn update_theme_setting(new_theme: String, cx: &mut App) {
    let mut settings = cx.global::<AppSettings>().clone();
    settings.theme = new_theme;

    // Attempt to save to the root of the workspace
    let path = "settings.json";

    if let Ok(json) = serde_json::to_string_pretty(&settings) {
        match std::fs::write(path, json) {
            Ok(_) => eprintln!("Successfully saved theme '{}' to {}", settings.theme, path),
            Err(e) => eprintln!("Failed to save settings: {}", e),
        }
    }

    cx.set_global(settings);
}
