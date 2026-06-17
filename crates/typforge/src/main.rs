//#![windows_subsystem = "windows"]

use anyhow::Result;
use std::sync::Arc;

use crate::{
    components::{menus::setup_menus, theme},
    key_bindings::bind_keys,
    settings::load_settings,
    typst_world::GpuiWorld,
};

use gpui::*;
use gpui_component::{
    Root,
    theme::{Theme, ThemeMode, ThemeRegistry},
};
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
                    titlebar: None,
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
