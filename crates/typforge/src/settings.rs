use gpui::{App, Global};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppSettings {
    pub theme: String,
    pub font_size: f32,
    pub last_folder_open: Option<String>,
    #[serde(default)] // Backwards compatibility for existing settings files
    pub recent_files: Vec<String>,
    #[serde(default = "default_recent_files_limit")]
    pub recent_files_limit: usize,
}

impl Global for AppSettings {}

fn default_recent_files_limit() -> usize {
    10 // Default to keeping 10 recent files
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "Tokyo Storm".into(),
            font_size: 16.0,
            last_folder_open: None,
            recent_files: Vec::new(),
            recent_files_limit: default_recent_files_limit(),
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

pub fn update_last_folder(path: String, cx: &mut App) {
    let mut settings = cx.global::<AppSettings>().clone();
    settings.last_folder_open = Some(path);
    let path = "settings.json";
    if let Ok(json) = serde_json::to_string_pretty(&settings) {
        let _ = std::fs::write(path, json);
    }
    cx.set_global(settings);
}

pub fn update_recent_files(path: String, cx: &mut App) {
    let mut settings = cx.global::<AppSettings>().clone();

    // Remove if already exists so it moves to index 0 (the top of the list)
    settings.recent_files.retain(|p| p != &path);
    settings.recent_files.insert(0, path);
    settings.recent_files.truncate(settings.recent_files_limit);

    let path_str = "settings.json";
    if let Ok(json) = serde_json::to_string_pretty(&settings) {
        let _ = std::fs::write(path_str, json);
    }
    cx.set_global(settings);

    // Reconstruct application menus to display the updated recent list
    crate::components::menus::setup_menus(cx);
}
