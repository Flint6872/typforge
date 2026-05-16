use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, InsertTextFormat};
use typst::syntax::{Source, SyntaxKind, parse};

// Import necessary items from typst_library crate.
use typst::Library;
use typst::foundations::Value;
use typst::syntax::{LinkedNode, Side};

/// Gathers completion items based on the current cursor position and context.
pub fn get_completions(
    source: &Source,
    byte_offset: usize,
    library: &Library,
) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    let text = source.text();

    let tree = parse(text);
    let root = LinkedNode::new(&tree);

    if let Some(leaf) = root.leaf_at(byte_offset, Side::Before) {
        if matches!(
            leaf.kind(),
            SyntaxKind::Str | SyntaxKind::LineComment | SyntaxKind::BlockComment | SyntaxKind::Raw
        ) {
            return Vec::new();
        }
    }

    let mut current_prefix_range = byte_offset..byte_offset;
    let mut i = byte_offset;
    while i > 0 {
        let prev_boundary = text[..i].char_indices().rev().next().map(|(idx, _)| idx);
        if prev_boundary.is_none() {
            break;
        }
        let char_idx = prev_boundary.unwrap();
        let c = text[char_idx..i].chars().next().unwrap();
        if c.is_alphanumeric() || c == '_' {
            current_prefix_range.start = char_idx;
        } else {
            break;
        }
        i = char_idx;
    }
    let prefix = &text[current_prefix_range.start..byte_offset];

    let mut is_after_dot = false;
    if current_prefix_range.start > 0 {
        if let Some(c) = text[..current_prefix_range.start].chars().next_back() {
            if c == '.' {
                is_after_dot = true;
            }
        }
    }

    if !is_after_dot {
        let keywords = [
            "let", "set", "show", "import", "include", "if", "else", "for", "while", "break",
            "continue", "return", "as", "in", "and", "or", "not", "block", "circle", "curve",
            "ellipse",
        ];
        for &kw in &keywords {
            if kw.starts_with(prefix) {
                items.push(CompletionItem {
                    label: kw.to_string(),
                    kind: Some(CompletionItemKind::KEYWORD),
                    ..Default::default()
                });
            }
        }
    }

    if !is_after_dot {
        for (name, binding) in library.global.scope().iter() {
            if name.starts_with(prefix) {
                // Get the Value from the binding
                let value = binding.read();

                let kind = match value {
                    Value::Func(_) => CompletionItemKind::FUNCTION,
                    Value::Module(_) => CompletionItemKind::MODULE,
                    Value::Type(_) => CompletionItemKind::CLASS,
                    Value::Auto => CompletionItemKind::CONSTANT,
                    Value::None => CompletionItemKind::KEYWORD,
                    Value::Bool(_)
                    | Value::Int(_)
                    | Value::Float(_)
                    | Value::Str(_)
                    | Value::Content(_)
                    | Value::Array(_)
                    | Value::Dict(_)
                    | Value::Symbol(_)
                    | Value::Datetime(_)
                    | Value::Decimal(_)
                    | Value::Duration(_)
                    | Value::Version(_)
                    | Value::Bytes(_)
                    | Value::Color(_)
                    | Value::Gradient(_)
                    | Value::Tiling(_)
                    | Value::Length(_)
                    | Value::Angle(_)
                    | Value::Ratio(_)
                    | Value::Relative(_)
                    | Value::Fraction(_)
                    | Value::Styles(_)
                    | Value::Args(_)
                    | Value::Label(_)
                    | Value::Dyn(_) => CompletionItemKind::CONSTANT,
                };

                let mut item = CompletionItem {
                    label: name.to_string(),
                    kind: Some(kind),
                    // Call docs() on the *read* value
                    detail: value.docs().map(|d: &'static str| d.to_string()),
                    ..Default::default()
                };

                if let Value::Func(_) = value {
                    item.insert_text = Some(format!("{}($0)", name));
                    item.insert_text_format = Some(InsertTextFormat::SNIPPET);
                }
                items.push(item);
            }
        }
    }

    items
}
