//! Mobile library entry point for TypForge (`libtypforge.so` on Android, `libtypforge.a` on iOS).

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
