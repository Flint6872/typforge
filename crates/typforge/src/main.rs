#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Result;

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

    // Replace `typforge` with your package name in Cargo.toml if different
    typforge::run_app(gpui_kit::application());
    Ok(())
}
