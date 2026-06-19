use std::path::PathBuf;

use gpui::{App, SharedString};
use gpui_component::{Theme, ThemeRegistry};

use crate::actions::ChangeTheme;
use crate::components::menus::setup_menus;
use crate::settings::update_theme_setting;

pub fn init(cx: &mut App) {
    // 1. Try to find the absolute path to be sure
    let mut themes_dir = std::env::current_dir().unwrap();

    // If you are running from workspace root, we need to go into the crate
    if themes_dir.ends_with("typforge0.0.1") {
        themes_dir = themes_dir.join("./themes");
    } else {
        // If you are already in the crate folder
        themes_dir = themes_dir.join("./themes");
    }

    eprintln!("Diagnostic: Checking for themes in {:?}", themes_dir);

    if !themes_dir.exists() {
        eprintln!("Error: Directory does not exist!");
    } else {
        // Count how many json files we see manually
        let files = std::fs::read_dir(&themes_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "json"))
            .count();
        eprintln!("Diagnostic: Found {} .json files in directory", files);
    }

    // 2. Register Action Handler
    cx.on_action(|action: &ChangeTheme, cx| {
        // 1. Update persistent settings and global state
        crate::settings::update_theme_setting(action.name.clone(), cx);

        // 2. Define the key explicitly as a &str for the map lookup
        let theme_name: &str = &action.name;

        // 3. Apply it
        if let Some(theme) = ThemeRegistry::global(cx).themes().get(theme_name).cloned() {
            Theme::global_mut(cx).apply_config(&theme);
            eprintln!("Theme changed to: {}", action.name);
        } else {
            eprintln!("Error: Could not find theme '{}' in registry", action.name);
        }
    });

    // 3. Start watching
    if let Err(err) = ThemeRegistry::watch_dir(themes_dir, cx, move |cx| {
        eprintln!("Callback: ThemeRegistry on_load triggered!");

        // Refresh the menus whenever themes are reloaded
        setup_menus(cx);

        // --- FIX: Apply the saved theme from settings.json here ---
        let settings = cx.global::<crate::settings::AppSettings>();
        let theme_name: SharedString = settings.theme.clone().into();

        if let Some(theme) = ThemeRegistry::global(cx).themes().get(&theme_name).cloned() {
            Theme::global_mut(cx).apply_config(&theme);
            eprintln!("Applied theme from settings: {}", theme_name);
        } else {
            eprintln!("Warning: Theme '{}' not found in registry", theme_name);
        }
    }) {
        eprintln!("Failed to watch: {}", err);
    }
    // Apply the saved default
    apply_settings_theme(cx);
}

pub fn apply_settings_theme(cx: &mut App) {
    let settings = cx.global::<crate::settings::AppSettings>();
    let theme_name: SharedString = settings.theme.clone().into();

    if let Some(theme) = ThemeRegistry::global(cx).themes().get(&theme_name).cloned() {
        Theme::global_mut(cx).apply_config(&theme);
        eprintln!("Applied theme from settings: {}", theme_name);
    }
}
