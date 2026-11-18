//! Library and entry points for TypForge (Desktop, Android `libtypforge.so`, iOS `libtypforge.a`).

#![allow(unused_imports)] // Temporarily allow during mobile setup
#![allow(unused_variables)] // Temporarily allow during mobile setup
#![deny(unsafe_op_in_unsafe_fn)] // Require `unsafe` for unsafe operations

use anyhow::Result;
use std::sync::Arc;

use gpui_kit::component::Root;
use gpui_kit::*;
use parking_lot::Mutex;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
use gpui_kit::assets::Assets;

#[cfg(any(target_os = "android", target_os = "ios"))]
extern crate gpui_mobile;

#[cfg(target_os = "android")]
use gpui_mobile::{android::jni, gpui::Application};

#[cfg(target_os = "ios")]
use gpui_mobile::ios::ffi;

mod actions;
mod components;
pub mod editor;
mod key_bindings;
pub mod package_manager;
mod panels;
mod ribbon;
mod settings;
pub mod typst_world;
pub mod workspace;

use crate::{
    components::{menus::setup_menus, theme},
    editor::init_editor_languages,
    key_bindings::bind_keys,
    settings::load_settings,
    typst_world::GpuiWorld,
    workspace::TypstNoteView,
};

// ═══════════════════════════════════════════════════════════════════════════
// Shared Application Initialization & Font Loading
// ═══════════════════════════════════════════════════════════════════════════

fn init_app_state(cx: &mut App) -> Arc<Mutex<GpuiWorld>> {
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
    Arc::new(Mutex::new(world))
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

// ═══════════════════════════════════════════════════════════════════════════
// Desktop Entry Point
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn run_desktop() -> Result<()> {
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
            let shared_world = init_app_state(cx);
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

// ═══════════════════════════════════════════════════════════════════════════
// Shared Mobile Window Creation (Android & iOS)
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(any(target_os = "android", target_os = "ios"))]
fn open_main_window(cx: &mut App) {
    log::info!("GPUI Mobile App context received.");

    let shared_world = init_app_state(cx);

    if let Err(err) = cx.open_window(
        WindowOptions {
            window_bounds: None,
            ..Default::default()
        },
        |window: &mut Window, cx: &mut App| {
            let typst_note_view = cx.new(|cx| {
                TypstNoteView::<crate::typst_world::GpuiWorld>::new(window, shared_world, cx)
            });

            cx.new(|cx| Root::new(typst_note_view, window, cx))
        },
    ) {
        log::error!("Failed to open mobile window: {err:#}");
    }

    cx.activate(true);
}

// ═══════════════════════════════════════════════════════════════════════════
// Android Entry Point
// ═══════════════════════════════════════════════════════════════════════════

/// Called by the `android-activity` crate on a dedicated native thread.
/// Does NOT return until the app is ready to exit.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn android_main(app: android_activity::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("TypforgeAndroid"),
    );

    jni::install_panic_hook();
    log::info!("android_main: entered");

    let _platform = jni::init_platform(&app);
    log::info!("android_main: platform initialised");

    let shared_platform = match jni::shared_platform() {
        Some(s) => s,
        None => {
            log::error!("android_main: shared_platform() returned None — aborting");
            return;
        }
    };

    log::info!("android_main: creating GPUI Application");
    Application::with_platform(shared_platform.into_rc()).run(|cx: &mut App| {
        open_main_window(cx);
    });

    log::info!("android_main: Application.run returned — activity will finish");
}

// ═══════════════════════════════════════════════════════════════════════════
// iOS Entry Point
// ═══════════════════════════════════════════════════════════════════════════

/// Minimal logger that routes Rust `log` crate messages through NSLog.
#[cfg(target_os = "ios")]
struct NsLogLogger;

#[cfg(target_os = "ios")]
impl log::Log for NsLogLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let msg = format!(
                "[{}] {}: {}",
                record.level(),
                record.target(),
                record.args()
            );
            nslog(&msg);
        }
    }

    fn flush(&self) {}
}

/// Call NSLog from Rust via raw FFI.
#[cfg(target_os = "ios")]
fn nslog(msg: &str) {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};

    unsafe extern "C" {
        fn NSLog(fmt: *mut AnyObject, ...);
    }

    unsafe {
        let c_msg = std::ffi::CString::new(msg).unwrap_or_default();
        let ns_msg: *mut AnyObject = msg_send![class!(NSString), alloc];
        let ns_msg: *mut AnyObject = msg_send![ns_msg, initWithUTF8String: c_msg.as_ptr()];
        let c_fmt = std::ffi::CString::new("%@").unwrap_or_default();
        let ns_fmt: *mut AnyObject = msg_send![class!(NSString), alloc];
        let ns_fmt: *mut AnyObject = msg_send![ns_fmt, initWithUTF8String: c_fmt.as_ptr()];
        NSLog(ns_fmt, ns_msg);
    }
}

/// Register the app's root view with the GPUI iOS platform.
///
/// Called from `main.m` before `gpui_ios_run_demo()`.
#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub extern "C" fn gpui_ios_register_app() {
    let _ = log::set_logger(&NsLogLogger).map(|()| log::set_max_level(log::LevelFilter::Info));

    std::panic::set_hook(Box::new(|info| {
        let msg = format!("GPUI PANIC: {info}");
        nslog(&msg);
    }));

    unsafe {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    ffi::set_app_callback(Box::new(|cx: &mut App| {
        open_main_window(cx);
    }));
}

/// Convenience entry point for the binary target (`main.rs`).
#[cfg(target_os = "ios")]
pub fn ios_main() {
    gpui_ios_register_app();
    ffi::run_app();
}
