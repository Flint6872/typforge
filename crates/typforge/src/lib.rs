//! Mobile library entry point for TypForge (`libtypforge.so` on Android, `libtypforge.a` on iOS).

#![allow(unused_imports)] // Temporarily allow during mobile setup
#![allow(unused_variables)] // Temporarily allow during mobile setup
#![deny(unsafe_op_in_unsafe_fn)] // Require `unsafe` for unsafe operations

// Only include gpui-mobile and its re-exported gpui when targeting Android or iOS
#[cfg(any(target_os = "android", target_os = "ios"))]
extern crate gpui_mobile;

#[cfg(any(target_os = "android", target_os = "ios"))]
use gpui_mobile::gpui::{self, App, Context, Render, Window, WindowOptions, div, prelude::*};

// Android-specific imports
#[cfg(target_os = "android")]
use gpui_mobile::android::jni;

// iOS-specific imports
#[cfg(target_os = "ios")]
use gpui_mobile::ios::ffi;

// ═══════════════════════════════════════════════════════════════════════════
// Shared Mobile Application Setup
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(any(target_os = "android", target_os = "ios"))]
fn open_main_window(cx: &mut App) {
    log::info!("GPUI Mobile App context received.");

    gpui_kit::component::init(cx);

    cx.open_window(
        WindowOptions {
            window_bounds: None,
            ..Default::default()
        },
        |window, cx| cx.new(|cx| Root::new(typst_note_view, window, cx)),
    )
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
    gpui::Application::with_platform(shared_platform.into_rc()).run(|cx: &mut App| {
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
