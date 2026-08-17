#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Result;
use std::sync::Arc;

use crate::{
    components::{menus::setup_menus, theme},
    key_bindings::bind_keys,
    settings::load_settings,
    typst_world::GpuiWorld,
};

use gpui::*;
use gpui_component::Root;
use gpui_component_assets::Assets;

use parking_lot::Mutex;

mod actions;
mod components;
pub mod editor;
mod key_bindings;
mod panels;
mod ribbon;
mod settings;
mod typst_world;

mod workspace;
use workspace::TypstNoteView;

fn main() -> Result<()> {
    // if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
    //     embed_resource::compile("assets/windows/resources.rc", embed_resource::NONE)
    //         .manifest_optional()
    //         .unwrap();
    // }
    // BAKE OPTION 1: Force XWayland/X11 backend on Linux to get native title bars
    #[cfg(target_os = "linux")]
    {
        if std::env::var("WAYLAND_DISPLAY").is_ok() && std::env::var("TYPFORGE_X11_FORCED").is_err()
        {
            if let Ok(current_exe) = std::env::current_exe() {
                let status = std::process::Command::new(current_exe)
                    .args(std::env::args().skip(1))
                    .env_remove("WAYLAND_DISPLAY")
                    .env("TYPFORGE_X11_FORCED", "1")
                    .status();

                if let Ok(code) = status {
                    std::process::exit(code.code().unwrap_or(0));
                }
            }
        }
    }

    gpui_platform::application()
        .with_assets(Assets)
        .run(|cx: &mut App| {
            // The closure receives &mut AppContext
            // Initialize GPUI components that might require a specific context setup
            //
            // cx.with_assets_directory(typforge::app::DEFAULT_ASSETS_DIRECTORY);
            // cx.run_migrations();

            gpui_component::init(cx);
            load_settings(cx);
            bind_keys(cx);
            theme::init(cx);
            theme::apply_settings_theme(cx);

            #[cfg(target_os = "windows")]
            cx.set_global(crate::components::menus::MenuState {
                has_active_file: true,
            });

            #[cfg(not(target_os = "windows"))]
            cx.set_global(crate::components::menus::MenuState {
                has_active_file: false,
            });

            setup_menus(cx);

            #[cfg(not(target_os = "macos"))]
            if let Some(menus) = cx.get_menus() {
                gpui_component::global_state::GlobalState::global_mut(cx).set_app_menus(menus);
            }

            // Theme::change(ThemeMode::Dark, None, cx);
            cx.set_global(typst_gpui::GpuiRegisteredFonts(
                std::collections::HashSet::new(),
            ));

            let fonts = load_fonts(cx);
            let mut world = GpuiWorld::new(fonts);
            world.set_source(String::new()); // Initialize empty source

            let shared_world = Arc::new(Mutex::new(world));
            let initial_bounds = Bounds::centered(None, size(px(1280.0), px(600.0)), cx);

            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(initial_bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("TypForge".into()),

                        // macOS: Makes the titlebar area transparent so your UI draws behind it
                        appears_transparent: cfg!(target_os = "macos"),

                        // macOS: Insets the red/yellow/green buttons
                        traffic_light_position: if cfg!(target_os = "macos") {
                            Some(point(px(16.0), px(16.0)))
                        } else {
                            None
                        },
                        ..Default::default()
                    }),
                    // Linux: Request native window borders
                    window_decorations: if cfg!(target_os = "linux") {
                        Some(WindowDecorations::Server)
                    } else {
                        None // Let macOS/Windows use their defaults
                    },
                    focus: true,
                    show: true,
                    kind: WindowKind::Normal,
                    is_resizable: true,
                    is_movable: true,
                    ..Default::default()
                },
                |window: &mut Window, cx: &mut App| {
                    // Explicitly type AppContext here
                    // First, create your main application view
                    let typst_note_view = cx.new(|cx| {
                        TypstNoteView::<crate::typst_world::GpuiWorld>::new(
                            window,
                            shared_world,
                            cx,
                        )
                    });

                    // Then, wrap it inside gpui_component::Root
                    cx.new(|cx| Root::new(typst_note_view, window, cx))
                },
            )
            .unwrap();
            cx.activate(true);
        });
    Ok(())
}

fn load_fonts(cx: &mut App) -> typst_kit::fonts::FontStore {
    let mut store = typst_kit::fonts::FontStore::new();

    // 1. Populate the store with both system and embedded fonts
    store.extend(typst_kit::fonts::system());
    store.extend(typst_kit::fonts::embedded());

    let preferred_families = [
        "New Computer Modern Math",
        "Libertinus Serif",
        "Segoe UI",
        "Inter",
        "Source Code Pro",
        "Noto Sans CJK JP",
    ];

    for family_name in preferred_families {
        // 2. Select the font from the book (store.book() dereferences to FontBook)
        if let Some(font_id) = store
            .book()
            .select(family_name, typst::text::FontVariant::default())
        {
            // 3. Retrieve and load the actual font data from the store
            if let Some(font) = store.font(font_id.into()) {
                println!("DEBUG: Found and loaded UI font: {}", family_name);
                let data = font.data().to_vec();

                // Add each found font to GPUI immediately
                let _ = cx
                    .text_system()
                    .add_fonts(vec![std::borrow::Cow::Owned(data)]);
            }
        }
    }

    store
}
