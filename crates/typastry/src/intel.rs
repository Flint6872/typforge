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

/// Helper function to detect if the target parameter context expects physical dimensions / units.
/// This runs on pure `&str` and is 100% editor-agnostic (no Rope required).
pub fn is_physical_dimension_context(content: &str, mut offset: usize) -> bool {
    let chars: Vec<char> = content.chars().collect();

    // Skip spaces backwards
    while offset > 0 {
        if let Some(c) = chars.get(offset - 1) {
            if c.is_whitespace() {
                offset -= 1;
                continue;
            }
        }
        break;
    }

    // Check for parameter assignment separator `:`
    if offset == 0 || chars.get(offset - 1) != Some(&':') {
        return false;
    }
    offset -= 1;

    // Skip spaces backwards
    while offset > 0 {
        if let Some(c) = chars.get(offset - 1) {
            if c.is_whitespace() {
                offset -= 1;
                continue;
            }
        }
        break;
    }

    // Scan backward to extract parameter name identifier
    let mut param_start = offset;
    while param_start > 0 {
        if let Some(prev_c) = chars.get(param_start - 1) {
            if prev_c.is_alphanumeric() || *prev_c == '_' || *prev_c == '-' {
                param_start -= 1;
                continue;
            }
        }
        break;
    }

    if param_start < offset {
        let param_name: String = chars[param_start..offset].iter().collect();
        matches!(
            param_name.as_str(),
            "size"
                | "margin"
                | "gap"
                | "gutter"
                | "width"
                | "height"
                | "radius"
                | "stroke"
                | "inset"
                | "outset"
                | "spacing"
        )
    } else {
        false
    }
}

/// Scans backwards in a source string to find the trigger boundary.
/// Returns: `(trigger_byte_offset, is_hash_command)`
pub fn get_trigger_info(content: &str, cursor: usize) -> (usize, bool) {
    if cursor == 0 {
        return (0, false);
    }
    let chars: Vec<char> = content.chars().collect();

    // 1. Find the start of the current alphanumeric word/identifier (allowing decimals)
    let mut word_start = cursor;
    while word_start > 0 {
        if let Some(prev_c) = chars.get(word_start - 1) {
            if prev_c.is_alphanumeric() || *prev_c == '_' || *prev_c == '-' || *prev_c == '.' {
                word_start -= 1;
                continue;
            }
        }
        break;
    }

    // 2. Check if the character immediately preceding the word is '#'
    if word_start > 0 && chars.get(word_start - 1) == Some(&'#') {
        return (word_start - 1, true);
    }

    (word_start, false)
}
