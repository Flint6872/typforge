#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Result;
use std::sync::Arc;

use crate::{
    components::{menus::setup_menus, theme},
    editor::init_editor_languages,
    key_bindings::bind_keys,
    settings::load_settings,
    typst_world::GpuiWorld,
};

use gpui_kit::assets::Assets;
use gpui_kit::component::Root;
use gpui_kit::*;

use parking_lot::Mutex;

mod actions;
mod components;
pub mod editor;
mod key_bindings;
pub mod package_manager;
mod panels;
mod ribbon;
mod settings;
mod typst_world;

mod workspace;
use workspace::TypstNoteView;

// The main entry point for desktop builds
#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn main() -> Result<()> {
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

    gpui_kit::application()
        .with_assets(Assets)
        .run(|cx: &mut App| {
            gpui_kit::component::init(cx);
            load_settings(cx);
            bind_keys(cx);
            theme::init(cx);
            theme::apply_settings_theme(cx);
            init_editor_languages(cx);

            cx.set_global(crate::components::menus::MenuState {
                has_active_file: cfg!(target_os = "windows"),
            });

            setup_menus(cx);

            #[cfg(not(target_os = "macos"))]
            if let Some(menus) = cx.get_menus() {
                gpui_kit::component::global_state::GlobalState::global_mut(cx).set_app_menus(menus);
            }

            cx.set_global(typst_gpui::GpuiRegisteredFonts(
                std::collections::HashSet::new(),
            ));

            let fonts = load_fonts(cx);
            let mut world = GpuiWorld::new(fonts);
            world.set_source(String::new());
            let shared_world = Arc::new(Mutex::new(world));

            let initial_bounds = Bounds::centered(None, size(px(1280.0), px(600.0)), cx);

            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(initial_bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("TypForge".into()),
                        appears_transparent: cfg!(target_os = "macos"),
                        traffic_light_position: if cfg!(target_os = "macos") {
                            Some(point(px(16.0), px(16.0)))
                        } else {
                            None
                        },
                        ..Default::default()
                    }),
                    window_decorations: if cfg!(target_os = "linux") {
                        Some(WindowDecorations::Server)
                    } else {
                        None
                    },
                    focus: true,
                    show: true,
                    kind: WindowKind::Normal,
                    is_resizable: true,
                    is_movable: true,
                    ..Default::default()
                },
                |window: &mut Window, cx: &mut App| {
                    let typst_note_view = cx.new(|cx| {
                        TypstNoteView::<crate::typst_world::GpuiWorld>::new(
                            window,
                            shared_world,
                            cx,
                        )
                    });

                    cx.new(|cx| Root::new(typst_note_view, window, cx))
                },
            )
            .unwrap();
            cx.activate(true);
        });
    Ok(())
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn main() -> Result<()> {
    // This `main` function is a stub for mobile.
    // On Android, `android_main` in `lib.rs` is the actual entry point.
    // On iOS, `ios_main` in `lib.rs` is called by Objective-C.
    // This allows `cargo check` to pass for the default target.
    eprintln!("This is the mobile placeholder main. The actual entry point is in lib.rs.");
    Ok(())
}

fn load_fonts(cx: &mut App) -> typst_kit::fonts::FontStore {
    let mut store = typst_kit::fonts::FontStore::new();

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
        if let Some(font_id) = store
            .book()
            .select(family_name, typst::text::FontVariant::default())
        {
            if let Some(font) = store.font(font_id.into()) {
                log::info!("Found and loaded UI font: {}", family_name);
                let data = font.data().to_vec();
                let _ = cx
                    .text_system()
                    .add_fonts(vec![std::borrow::Cow::Owned(data)]);
            }
        }
    }

    store
}

// iOS-specific main function, called from Objective-C main.m if needed.
// #[cfg(target_os = "ios")]
// pub fn ios_main() {
//     typforge::gpui_ios_register_app(); // Call the FFI entry point from lib.rs
//     // The actual run loop for iOS starts after gpui_ios_register_app()
//     // has registered its callback, typically in gpui_mobile::ios::ffi::run_app().
// }
