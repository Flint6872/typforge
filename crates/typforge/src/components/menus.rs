use std::{collections::HashMap, eprintln};

use crate::{
    actions::{self, ChangeTheme},
    editor::editor_panel,
};
use gpui::{App, Global, Menu, MenuItem};
use gpui_component::{ThemeRegistry, ThemeSet};

// 1. Define global state to track if a file is active
pub struct MenuState {
    pub has_active_file: bool,
}

impl Global for MenuState {}

// 2. Helper function to update MenuState and refresh the application menus
pub fn update_menu_file_state(has_file: bool, cx: &mut App) {
    cx.set_global(MenuState {
        has_active_file: has_file,
    });
    setup_menus(cx);
}

pub fn setup_menus(cx: &mut App) {
    let registry = ThemeRegistry::global(cx);

    // Get themes from registry first, they have canonical names.
    let mut theme_map: HashMap<String, String> = registry
        .themes()
        .iter()
        .map(|(name, config)| {
            (name.to_string(), config.name.to_string()) // Store original name and the registry key
        })
        .collect();

    // Manually scan directory for theme JSONs
    let mut themes_dir = std::env::current_dir().unwrap();
    if themes_dir.ends_with("typforge0.0.1") {
        themes_dir = themes_dir.join("./themes");
    } else {
        themes_dir = themes_dir.join("./themes");
    }

    if let Ok(entries) = std::fs::read_dir(themes_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "json") {
                if let Ok(file_content) = std::fs::read_to_string(&path) {
                    if let Ok(theme_set) = serde_json::from_str::<ThemeSet>(&file_content) {
                        for theme in theme_set.themes {
                            let display_name = theme.name.clone();
                            let registry_key = display_name.clone(); // Assuming name in JSON is the key

                            if !theme_map.contains_key(&registry_key.to_string()) {
                                theme_map
                                    .insert(registry_key.to_string(), display_name.to_string());
                                eprintln!("Found custom theme from file: '{}'", display_name);
                            }
                        }
                    }
                }
            }
        }
    }

    // Collect the display names for the menu
    let mut display_names: Vec<String> = theme_map.values().cloned().collect();
    display_names.sort();

    // Hardcoded fallback for safety (optional, but good for debugging)
    if display_names.len() <= 2 && display_names.iter().all(|n| n.starts_with("Default")) {
        display_names.extend(vec![
            "Tokyo Night".to_string(),
            "Twilight".to_string(),
            "Catppuccin Latte".to_string(), // Ensure exact name here
        ]);
        display_names.sort();
        display_names.dedup();
    }

    let theme_items = display_names
        .into_iter()
        .map(|name| MenuItem::action(name.clone(), ChangeTheme { name: name.clone() }))
        .collect::<Vec<_>>();

    let settings = cx.global::<crate::settings::AppSettings>();

    let recent_files_items = if settings.recent_files.is_empty() {
        vec![MenuItem::action("No Recent Files", actions::FileOpen).disabled(true)]
    } else {
        settings
            .recent_files
            .iter()
            .map(|path| {
                let path_buf = std::path::PathBuf::from(path);
                let display_name = path_buf
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.clone());

                MenuItem::action(
                    display_name,
                    actions::OpenRecentFiles { path: path.clone() }, // <--- Correct parameterized action
                )
            })
            .collect::<Vec<_>>()
    };

    let has_file = cx
        .try_global::<MenuState>()
        .map_or(false, |state| state.has_active_file);

    cx.set_menus(vec![
        // File Menu
        Menu {
            name: "File".into(),
            items: vec![
                MenuItem::action("New File", actions::FileNew),
                MenuItem::action("Open File...", actions::FileOpen),
                MenuItem::action("Open Folder...", actions::FolderOpen),
                MenuItem::submenu(Menu {
                    name: "Open Recent".into(),
                    items: recent_files_items,
                    disabled: false,
                }),
                MenuItem::separator(),
                MenuItem::action("Save", actions::FileSave).disabled(!has_file),
                MenuItem::action("Save As...", actions::FileSaveAs).disabled(!has_file),
                MenuItem::action("Close File", actions::FileClose).disabled(!has_file),
                MenuItem::separator(),
                MenuItem::action("Package Manager", actions::PackageManager).disabled(true),
                MenuItem::submenu(Menu {
                    name: "Export To".into(),
                    items: vec![
                        MenuItem::action("PDF", actions::FileExportPdf).disabled(!has_file),
                        MenuItem::action("Word Document", actions::FileExportDocx)
                            .disabled(!has_file),
                    ],
                    disabled: false,
                }),
                MenuItem::separator(),
                MenuItem::action("Quit", actions::FileQuit),
            ],
            disabled: false,
        },
        // // Edit Menu
        // Menu {
        //     name: "Edit".into(),
        //     items: vec![
        //         MenuItem::action("Undo", actions::EditUndo),
        //         MenuItem::action("Redo", actions::EditRedo),
        //         MenuItem::separator(),
        //         MenuItem::action("Cut", actions::EditCut),
        //         MenuItem::action("Copy", actions::EditCopy),
        //         MenuItem::action("Paste", actions::EditPaste),
        //     ],
        //     disabled: false,
        // },
        // // View Menu
        // Menu {
        //     name: "View".into(),
        //     items: vec![
        //         MenuItem::action("Toggle Sidebar", actions::ViewToggleSidebar),
        //         MenuItem::separator(),
        //         MenuItem::action("Zoom In", actions::ZoomIn),
        //         MenuItem::action("Zoom Out", actions::ZoomOut),
        //         MenuItem::action("Reset Zoom", actions::ResetZoom),
        //     ],
        //     disabled: false,
        // },
        // ... Theme Change ...
        Menu {
            name: "Theme".into(),
            items: theme_items,
            disabled: false,
        },
        // Help Menu
        Menu {
            name: "Help".into(),
            items: vec![MenuItem::action("About TypstNote", actions::HelpAbout)],
            disabled: false,
        },
    ]);
}
