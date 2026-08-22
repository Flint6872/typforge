#![cfg(feature = "intel")]
use typst::syntax::Side;
use typst::syntax::Source;
pub use typst_ide::{Completion, IdeWorld, Tooltip, autocomplete, tooltip};
use typst_layout::PagedDocument;

/// A portable, serializable classification for completion options.
/// Useful for other editors (Python, TypeScript/WASM, terminal) to map cleanly to their own UI.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum CompletionKind {
    Func,
    Type,
    Param,
    Constant,
    Symbol,
    Unit, // For coaching suggestions
    Text,
}

/// A clean, pre-processed completion payload ready for editor consumption.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct TypastryCompletion {
    pub label: String,
    pub apply: String,
    pub detail: Option<String>,
    pub kind: CompletionKind,
}

/// Retrieves raw, context-aware completions from `typst-ide`.
pub fn get_completions(
    world: &dyn IdeWorld,
    document: Option<&PagedDocument>,
    source: &Source,
    cursor_index: usize,
    explicit: bool,
) -> Vec<Completion> {
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
    tooltip(world, document, source, cursor_index, Side::After)
}

/// High-level, modular completions engine. Fetches, filters, and applies
/// coaching rules on pure Rust primitives, making autocomplete powerful for any platform.
pub fn get_enhanced_completions(
    world: &dyn IdeWorld,
    document: Option<&PagedDocument>,
    source: &Source,
    cursor_index: usize,
) -> Vec<TypastryCompletion> {
    let content = source.text();
    let mut results = Vec::new();

    // 1. Fetch raw underlying compiler suggestion list
    let raw_completions = get_completions(world, document, source, cursor_index, false);

    // 2. Discover cursor contexts
    let (trigger_offset, is_hash_command) = get_trigger_info(content, cursor_index);
    let typed_prefix = if cursor_index > trigger_offset {
        let start = if is_hash_command {
            trigger_offset + 1
        } else {
            trigger_offset
        };
        if cursor_index > start {
            content
                .chars()
                .skip(start)
                .take(cursor_index - start)
                .collect::<String>()
                .to_lowercase()
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // 3. Process & filter compiler suggestions
    let filtered: Vec<TypastryCompletion> = raw_completions
        .into_iter()
        .filter(|c| {
            if typed_prefix.is_empty() {
                true
            } else {
                c.label
                    .to_string()
                    .to_lowercase()
                    .starts_with(&typed_prefix)
            }
        })
        .map(|c| {
            let label = c.label.to_string();
            let raw_apply_text = c
                .apply
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or_else(|| label.clone());

            // Simplify autocomplete text to raw plain text:
            let apply_text = if raw_apply_text.contains('(') {
                let base = raw_apply_text.split('(').next().unwrap_or(&label);
                format!("{}()", base)
            } else {
                raw_apply_text.replace("${}", "").replace("${1:}", "")
            };

            let kind = match c.kind {
                typst_ide::CompletionKind::Func => CompletionKind::Func,
                typst_ide::CompletionKind::Type => CompletionKind::Type,
                typst_ide::CompletionKind::Param => CompletionKind::Param,
                typst_ide::CompletionKind::Constant => CompletionKind::Constant,
                typst_ide::CompletionKind::Symbol(_) => CompletionKind::Symbol,
                _ => CompletionKind::Text,
            };

            TypastryCompletion {
                label,
                apply: apply_text,
                detail: c.detail.map(|s| s.to_string()),
                kind,
            }
        })
        .collect();

    results.extend(filtered);

    // 4. --- COACHING: PHYSICAL DIMENSIONS & UNITS HEURISTICS ---
    if !typed_prefix.is_empty() && is_physical_dimension_context(content, trigger_offset) {
        if let Ok(_number) = typed_prefix.parse::<f64>() {
            let units = ["pt", "em", "cm", "mm"];
            for unit in units {
                let suggested_text = format!("{}{}", typed_prefix, unit);
                results.insert(
                    0, // Push coaching recommendations directly to the top!
                    TypastryCompletion {
                        label: suggested_text.clone(),
                        apply: suggested_text,
                        detail: Some(format!("Coaching: Insert explicit length ({})", unit)),
                        kind: CompletionKind::Unit,
                    },
                );
            }
        }
    }

    results
}

/// Helper function to detect if the target parameter context expects physical dimensions / units.
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
