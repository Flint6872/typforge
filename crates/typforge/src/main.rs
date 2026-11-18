#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Result;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn main() -> Result<()> {
    typforge::run_desktop()
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn main() -> Result<()> {
    // On Android, `android_main` in `lib.rs` is the actual entry point.
    // On iOS, `gpui_ios_register_app` / `ios_main` in `lib.rs` is called by Objective-C.
    eprintln!("This is the mobile placeholder main. The actual entry point is in lib.rs.");
    Ok(())
}
