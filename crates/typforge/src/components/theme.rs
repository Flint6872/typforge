use std::path::PathBuf;

use gpui::{App, SharedString};
use gpui_component::{Theme, ThemeRegistry};

use crate::actions::ChangeTheme;
use crate::components::menus::setup_menus;

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
        let theme_name: SharedString = action.name.clone().into();
        if let Some(theme) = ThemeRegistry::global(cx).themes().get(&theme_name).cloned() {
            Theme::global_mut(cx).apply_config(&theme);
        }
    });

    // 3. Start watching
    if let Err(err) = ThemeRegistry::watch_dir(themes_dir, cx, move |cx| {
        eprintln!("Callback: ThemeRegistry on_load triggered!");

        let registry = ThemeRegistry::global(cx);
        let names: Vec<_> = registry.themes().keys().map(|k| k.to_string()).collect();
        eprintln!("Loaded Theme Names: {:?}", names);

        // Ensure we are applying the logic to the current context
        setup_menus(cx);

        // Apply the default
        let default_theme_name: SharedString = "Tokyo Storm".into();
        if let Some(theme) = ThemeRegistry::global(cx)
            .themes()
            .get(&default_theme_name)
            .cloned()
        {
            Theme::global_mut(cx).apply_config(&theme);
            eprintln!("Default theme applied: {}", default_theme_name);
        }
    }) {
        eprintln!("Failed to watch: {}", err);
    }
}
