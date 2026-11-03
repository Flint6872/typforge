// crates/typforge/src/ribbon/icon.rs
use gpui_kit::component::IconNamed;
use rust_embed::RustEmbed;
use std::sync::Arc;

#[derive(RustEmbed)]
#[folder = "../../assets/ribbon_icons"]
pub struct RibbonIcons;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RibbonIconName {
    Bold,
    Italic,
    Underline,
    Pin,
    ChevronDown,
    ChevronUp,
    Table,
}

impl RibbonIconName {
    pub fn load_svg(self) -> Option<Arc<[u8]>> {
        let path = self.path();
        RibbonIcons::get(&path).map(|f| Arc::<[u8]>::from(f.data.into_owned()))
    }
}

impl IconNamed for RibbonIconName {
    fn path(self) -> gpui_kit::SharedString {
        match self {
            RibbonIconName::Bold => "letter-b.svg",
            RibbonIconName::Italic => "italic.svg",
            RibbonIconName::Underline => "underline.svg",
            RibbonIconName::Pin => "pin.svg",
            RibbonIconName::ChevronDown => "chevrons-down.svg",
            RibbonIconName::ChevronUp => "chevrons-up.svg",
            RibbonIconName::Table => "table.svg",
        }
        .into()
    }
}
