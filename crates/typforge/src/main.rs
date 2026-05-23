#![windows_subsystem = "windows"]

use anyhow::Result;
use std::sync::Arc;

use crate::{
    components::{lsp::LspClient, menus::setup_menus, theme},
    key_bindings::bind_keys,
    settings::load_settings,
    typst_world::GpuiWorld,
};

use gpui::*;
use gpui_component::{
    Root,
    theme::{Theme, ThemeMode, ThemeRegistry},
};

use typst_kit::fonts::Fonts;

use parking_lot::Mutex;

mod actions;
mod components;
pub mod editor;
mod key_bindings;
mod panels;
mod settings;
mod typst_world;

mod workspace;
use workspace::TypstNoteView;

fn main() -> Result<()> {
    // 1. Create a multi-threaded Tokio runtime
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");

    // 2. Enter the runtime context.
    // This allows tokio::spawn and tokio::io::duplex to work
    // on this thread and its children.
    let _guard = rt.enter();

    gpui_platform::application().run(|cx: &mut App| {
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
        let initial_lsp_content = String::new();
        let mut world = GpuiWorld::new(fonts);
        world.set_source(initial_lsp_content.clone()); // Initialize it with some content

        let shared_world = Arc::new(Mutex::new(world));

        // Initialize our clean LSP Client
        let (lsp_client, diagnostics_rx, responses_rx) = LspClient::new(shared_world.clone());
        let lsp_client = Arc::new(lsp_client);

        lsp_client.initialize(typstography::ClientCapabilities::default());

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
                    TypstNoteView::new(
                        window,
                        shared_world,
                        lsp_client.clone(),
                        diagnostics_rx,
                        responses_rx,
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

fn load_fonts(cx: &mut App) -> Fonts {
    let mut searcher = typst_kit::fonts::FontSearcher::new();
    searcher.include_system_fonts(true);
    let all_typst_fonts_result = searcher.search();

    // let loaded_gpui_font_data: Option<Vec<u8>> = None;

    // Use the font book to find a preferred UI font (e.g., "Segoe UI" or "Inter")
    // Note: Typst font families are case-insensitive.
    let preferred_families = [
        "New Computer Modern Math",
        "Libertinus Serif",
        "Segoe UI",
        "Inter",
        "Source Code Pro",
        "Noto Sans CJK JP",
    ]; // Added "Noto Sans CJK JP" for broader coverage

    for family_name in preferred_families {
        // Use FontBook::select to find a font by family name and variant
        // `FontVariant::default()` usually means "normal" weight, "normal" style.
        if let Some(font_id) = all_typst_fonts_result
            .book
            .select(family_name, typst::text::FontVariant::default())
        {
            // Get the corresponding FontSlot using its index (FontId is just an index)
            if let Some(font_slot) = all_typst_fonts_result.fonts.get(font_id) {
                // font_id is already usize, no need to_usize()
                // Now, and only now, call .get() on this specific font_slot to load its data.
                if let Some(font) = font_slot.get() {
                    println!("DEBUG: Found and loaded UI font: {}", family_name);
                    let data = font.data().to_vec();
                    // Add each found font to GPUI immediately
                    let _ = cx
                        .text_system()
                        .add_fonts(vec![std::borrow::Cow::Owned(data)]);
                }
            }
        }
    }

    all_typst_fonts_result
}
