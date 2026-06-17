// typforge-core/src/intel.rs

use typst::syntax::Side;
use typst::syntax::Source;
pub use typst_ide::{Completion, CompletionKind, IdeWorld, Tooltip, autocomplete, tooltip};
use typst_layout::PagedDocument;

/// Retrieves a list of completions at the specified cursor position.
pub fn get_completions(
    world: &dyn IdeWorld,
    document: Option<&PagedDocument>,
    source: &Source,
    cursor_index: usize,
    explicit: bool,
) -> Vec<Completion> {
    // Passes the document to allow for context-aware completions (e.g. references)
    autocomplete(world, document, source, cursor_index, explicit)
        .map(|(_, completions)| completions)
        .unwrap_or_default()
}

/// Retrieves tooltip/documentation at the specified cursor position.
pub fn get_hover_info(
    world: &dyn IdeWorld,
    document: Option<&PagedDocument>,
    source: &Source,
    cursor_index: usize,
) -> Option<Tooltip> {
    // tooltip requires the document to resolve references and labels
    tooltip(world, document, source, cursor_index, Side::After)
}
