// crates/typforge/src/lib.rs

//! Mobile library entry point for TypForge (`libtypforge.so` on Android, `libtypforge.a` on iOS).

#![allow(unused_imports)] // Temporarily allow during mobile setup
#![allow(unused_variables)] // Temporarily allow during mobile setup
#![deny(unsafe_op_in_unsafe_fn)] // Require `unsafe` for unsafe operations

// Only include gpui-mobile and its re-exported gpui when targeting Android or iOS
#[cfg(any(target_os = "android", target_os = "ios"))]
extern crate gpui_mobile;

#[cfg(any(target_os = "android", target_os = "ios"))]
use gpui_mobile::gpui::{self, App, WindowOptions, prelude::*}; // Use gpui-mobile's re-exported gpui

use log;

// Android-specific imports
#[cfg(target_os = "android")]
use {android_activity, android_logger, gpui_mobile::android::jni};

// iOS-specific imports
#[cfg(target_os = "ios")]
use gpui_mobile::ios::ffi;

// ═══════════════════════════════════════════════════════════════════════════
// Shared Mobile Application Setup
// ═══════════════════════════════════════════════════════════════════════════

// This function should only be compiled for mobile targets
#[cfg(any(target_os = "android", target_os = "ios"))]
fn run_mobile_app(app: gpui::Application) {
    app.run(move |cx: &mut gpui::AppContext| {
        log::info!("GPUI Mobile App context received.");

        cx.open_window(
            WindowOptions::default(),
            |window: &mut gpui::Window, cx: &mut gpui::ViewContext| {
                cx.new_view(|_cx| {
                    gpui::div()
                        .size_full()
                        .flex()
                        .justify_center()
                        .items_center()
                        .child("Hello from Typforge Mobile!")
                        .text_color(gpui::rgb(0xFFFFFF))
                        .into_any_element()
                })
            },
        );
        cx.activate(true);
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// Android Entry Point
// ═══════════════════════════════════════════════════════════════════════════

/// Called by the `android-activity` crate on a dedicated native thread.
/// Does NOT return until the app is ready to exit.
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn android_main(app: android_activity::AndroidApp) {
    // Logger first — so everything after this is visible in logcat.
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("TypforgeAndroid"),
    );

    // Panic hook — routes panics to logcat instead of silently aborting.
    jni::install_panic_hook();

    log::info!("android_main: entered");

    // Initialise the global AndroidApp + AndroidPlatform.
    let _platform = jni::init_platform(&app);
    log::info!("android_main: platform initialised");

    // Get a SharedPlatform (Rc-compatible wrapper around the global
    // Arc<AndroidPlatform>) so we can hand it to GPUI.
    let shared_platform = match jni::shared_platform() {
        Some(s) => s,
        None => {
            log::error!("android_main: shared_platform() returned None — aborting");
            return;
        }
    };

    log::info!("android_main: creating GPUI Application");
    let app = gpui::Application::with_platform(shared_platform.into_rc());

    // `Application::run` on Android blocks by driving the native event loop.
    // The `run_mobile_app` function (which opens the window) is deferred until
    // `MainEvent::InitWindow` delivers a native surface.
    run_mobile_app(app);

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
            unsafe {
                nslog(&msg);
            }
        }
    }
    fn flush(&self) {}
}

/// Call NSLog from Rust via raw FFI.
#[cfg(target_os = "ios")]
unsafe fn nslog(msg: &str) {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    unsafe {
        // Safety: This is an FFI call to NSLog, which is variadic.
        extern "C" {
            fn NSLog(fmt: *mut AnyObject, ...);
        }
        let c_msg = std::ffi::CString::new(msg).unwrap_or_default();
        let ns_msg: *mut AnyObject = msg_send![class!(NSString), alloc];
        let ns_msg: *mut AnyObject = msg_send![ns_msg, initWithUTF8String: c_msg.as_ptr()];
        let c_fmt = std::ffi::CString::new("%@").unwrap_or_default();
        let ns_fmt: *mut AnyObject = msg_send![class!(NSString), alloc];
        let ns_fmt: *mut AnyObject = msg_send![ns_fmt, initWithUTF8String: c_fmt.as_ptr()];
        NSLog(ns_fmt, ns_msg);
    }
}

/// Register the example app's root view with the GPUI iOS platform.
///
/// This is called from `main.m` **before** `gpui_ios_run_demo()` so that
/// when the GPUI run loop starts it knows which view to create.
///
/// The symbol lives in the example crate's static lib which is force-loaded
/// alongside `libgpui_mobile.a` by the Xcode linker.
#[cfg(target_os = "ios")]
#[no_mangle]
pub extern "C" fn gpui_ios_register_app() {
    // Set up Rust logging → NSLog so log::info! etc. appear in devicectl --console.
    let _ = log::set_logger(unsafe { &NsLogLogger })
        .map(|()| log::set_max_level(log::LevelFilter::Info));

    // Panic hook → NSLog so panics are visible.
    std::panic::set_hook(Box::new(|info| {
        let msg = format!("GPUI PANIC: {info}");
        unsafe {
            nslog(&msg);
        }
    }));

    // std::env::set_var is an unsafe function
    unsafe {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    ffi::set_app_callback(Box::new(|cx: &mut gpui::AppContext| {
        log::info!("GPUI iOS App context received.");
        let app = gpui::Application::with_platform(gpui_mobile::current_platform(false));
        run_mobile_app(app);
    }));
}

/// Convenience entry point for the binary target (`main.rs`).
#[cfg(target_os = "ios")]
pub fn ios_main() {
    unsafe {
        gpui_ios_register_app();
    } // Safety: FFI call
    gpui_mobile::ios::ffi::run_app();
}
