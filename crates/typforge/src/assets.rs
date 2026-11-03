// // crates/typforge/src/assets.rs
// use gpui_kit::asset_source::AssetSource; // Assuming gpui_kit re-exports this
// use std::borrow::Cow;
// use std::sync::Arc;
//
// #[derive(rust_embed::RustEmbed)]
// #[folder = "assets"] // This points to your `typforge/assets` directory
// // Add all your custom icons here
// #[include = "ribbon_icons/letter-b.svg"]
// #[include = "ribbon_icons/italic.svg"]
// #[include = "ribbon_icons/underline.svg"]
// #[include = "ribbon_icons/pin.svg"]
// #[include = "ribbon_icons/chevrons-down.svg"]
// #[include = "ribbon_icons/chevrons-up.svg"]
// #[include = "ribbon_icons/font_color.svg"]
// // ... add any other custom SVGs you have or will have ...
// pub struct AppAssets; // Renamed to AppAssets to avoid conflict with gpui_kit_assets::Assets
//
// impl AssetSource for AppAssets {
//     fn load_asset(&self, path: &str) -> Option<Arc<[u8]>> {
//         Self::get(path).map(|f| Arc::<[u8]>::from(f.data.into_owned()))
//     }
// }
