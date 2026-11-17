//! Mobile library entry point for TypForge (`libtypforge.so` on Android, `libtypforge.a` on iOS).
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
use std::sync::Arc;

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn gpui_android_main() {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("TypforgeAndroid"),
    );
    log::info!("Starting Typforge Android main");

    let platform = gpui_mobile::current_platform(false);
    let _ = platform;

    // TODO: Wire up gpui_mobile::gpui::Application::with_platform(platform).run(...)
    // matching your gpui-mobile fork's android_app example.
}

#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub extern "C" fn gpui_ios_register_app() {
    log::info!("Registering Typforge iOS app");

    // TODO: Register the root view callback with gpui_mobile::ios
    // matching your gpui-mobile fork's ios example.
}

/// Shared entry point called by desktop (`main.rs`), Android, and iOS.
pub fn run_app(app: Application) {
    app.with_assets(Assets).run(|cx: &mut App| {
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

        // Desktop uses custom bounds & titlebars; mobile uses full-screen defaults
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        let window_options = {
            let initial_bounds = Bounds::centered(None, size(px(1280.0), px(600.0)), cx);
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
            }
        };

        #[cfg(any(target_os = "android", target_os = "ios"))]
        let window_options = WindowOptions::default();

        cx.open_window(window_options, |window: &mut Window, cx: &mut App| {
            let typst_note_view =
                cx.new(|cx| TypstNoteView::<GpuiWorld>::new(window, shared_world, cx));
            cx.new(|cx| Root::new(typst_note_view, window, cx))
        })
        .expect("Failed to open main TypForge window");

        cx.activate(true);
    });
}

fn load_fonts(cx: &mut App) -> typst_kit::fonts::FontStore {
    let mut store = typst_kit::fonts::FontStore::new();

    // Note: On mobile, embedded fonts are critical since desktop system fonts won't exist
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

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn gpui_android_main() {
    std::env::set_var("RUST_BACKTRACE", "1");
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("TypforgeAndroid"),
    );
    log::info!("Starting Typforge Android main");

    let platform = gpui_mobile::current_platform(false);
    run_app(Application::with_platform(platform));
}

#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub extern "C" fn gpui_ios_register_app() {
    std::env::set_var("RUST_BACKTRACE", "1");
    log::info!("Registering Typforge iOS app");

    // Note: Depending on your gpui-mobile fork's iOS lifecycle, you either call
    // run_app(Application::with_platform(gpui_mobile::current_platform(false))) here
    // or pass a closure into gpui_mobile::ios::register_app(|| { ... }).
    let platform = gpui_mobile::current_platform(false);
    run_app(Application::with_platform(platform));
}
