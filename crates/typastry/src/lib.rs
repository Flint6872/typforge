pub mod edit;
pub mod inspector;
#[cfg(feature = "spellcheck")]
pub mod spellcheck;
pub mod syntax;
pub mod wysiwyg;

#[cfg(feature = "format")]
pub mod format;

#[cfg(feature = "intel")]
pub mod intel;

#[cfg(feature = "intel")]
pub use typst_ide::IdeWorld;

#[cfg(feature = "intel")]
pub use inspector::TypastryJump;

pub use inspector::{CursorContext, TypastryInspector};
