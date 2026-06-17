// typforge-core/src/edit.rs

use std::ops::Range;
use typst_syntax::{LinkedNode, Side, SyntaxKind, parse};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum EditAction {
    ToggleBold,
    ToggleItalic,
    SetFont(String),
    SetFontSize(f64),
    SetTextColor(String),
    InsertGrid { rows: usize, cols: usize },
    SetPaper(String),
    SetFlipped(bool),
    SetColumns(usize),
    SetMargin(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub range: Range<usize>,
    pub new_text: String,
    pub new_selection: Range<usize>,
}

/// Applies an EditAction to the source text.
/// Returns the localized TextEdit to apply.
pub fn apply_edit_action(content: &str, selection: Range<usize>, action: &EditAction) -> TextEdit {
    let start = selection.start.min(content.len());
    let end = selection.end.min(content.len());
    let selection_clamped = start..end;

    match action {
        // --- 1. TEXT PARAMETERS (Smart Toggles & Mergers) ---
        EditAction::ToggleBold => {
            toggle_wrapper_ast(content, selection_clamped, SyntaxKind::Strong)
        }
        EditAction::ToggleItalic => {
            toggle_wrapper_ast(content, selection_clamped, SyntaxKind::Emph)
        }
        EditAction::SetFont(font_family) => apply_text_param_ast(
            content,
            selection_clamped,
            "font",
            &format!("\"{}\"", font_family),
        ),
        EditAction::SetFontSize(size) => {
            apply_text_param_ast(content, selection_clamped, "size", &format!("{}pt", size))
        }
        EditAction::SetTextColor(color) => {
            apply_text_param_ast(content, selection_clamped, "fill", color)
        }

        // --- 2. COMPLEX ELEMENTS (Cursor Injections) ---
        EditAction::InsertGrid { rows, cols } => {
            let mut grid_markup = String::new();
            grid_markup.push_str("\n#grid(\n");
            let col_frs = vec!["1fr"; *cols].join(", ");
            grid_markup.push_str(&format!("  columns: ({}),\n", col_frs));
            grid_markup.push_str("  gutter: 1em,\n");

            for r in 1..=*rows {
                grid_markup.push_str("  ");
                let mut row_cells = Vec::new();
                for c in 1..=*cols {
                    row_cells.push(format!("[Cell R{}C{}]", r, c));
                }
                grid_markup.push_str(&row_cells.join(", "));
                grid_markup.push_str(",\n");
            }
            grid_markup.push_str(")\n");

            TextEdit {
                range: start..start,
                new_text: grid_markup.clone(),
                new_selection: start..(start + grid_markup.len()),
            }
        }

        // --- 3. PAGE PARAMETERS (Global Directive Setters) ---
        EditAction::SetPaper(paper) => {
            let (edit_range, edit_text) =
                update_or_insert_page_rule_ast(content, "paper", &format!("\"{}\"", paper));
            let new_selection = adjust_selection(edit_range.clone(), edit_text.len(), selection);
            TextEdit {
                range: edit_range,
                new_text: edit_text,
                new_selection,
            }
        }
        EditAction::SetFlipped(flipped) => {
            let (edit_range, edit_text) =
                update_or_insert_page_rule_ast(content, "flipped", &flipped.to_string());
            let new_selection = adjust_selection(edit_range.clone(), edit_text.len(), selection);
            TextEdit {
                range: edit_range,
                new_text: edit_text,
                new_selection,
            }
        }
        EditAction::SetColumns(cols) => {
            let (edit_range, edit_text) =
                update_or_insert_page_rule_ast(content, "columns", &cols.to_string());
            let new_selection = adjust_selection(edit_range.clone(), edit_text.len(), selection);
            TextEdit {
                range: edit_range,
                new_text: edit_text,
                new_selection,
            }
        }
        EditAction::SetMargin(margin) => {
            let (edit_range, edit_text) =
                update_or_insert_page_rule_ast(content, "margin", &format!("({})", margin));
            let new_selection = adjust_selection(edit_range.clone(), edit_text.len(), selection);
            TextEdit {
                range: edit_range,
                new_text: edit_text,
                new_selection,
            }
        }
    }
}

/// Helper function to find a formatting ancestor node fully enclosing the target range
fn find_formatting_node<'a>(
    root: &'a LinkedNode<'a>,
    range: Range<usize>,
    kind: SyntaxKind,
) -> Option<LinkedNode<'a>> {
    // Look after the start position to get inside the formatted range
    let leaf = root
        .leaf_at(range.start, Side::After)
        .or_else(|| root.leaf_at(range.start, Side::Before))?;
    let mut current = Some(leaf);
    while let Some(node) = current {
        if node.kind() == kind && node.range().start <= range.start && node.range().end >= range.end
        {
            return Some(node);
        }
        current = node.parent().cloned();
    }
    None
}

/// Toggles bold or italic using synchronous syntax trees.
fn toggle_wrapper_ast(content: &str, range: Range<usize>, kind_to_toggle: SyntaxKind) -> TextEdit {
    let tree = parse(content);
    let root = LinkedNode::new(&tree);

    // 1. Check if the selection is already wrapped
    if let Some(node) = find_formatting_node(&root, range.clone(), kind_to_toggle) {
        let node_range = node.range();
        let inner_text = &content[node_range.start + 1..node_range.end - 1]; // Offset 1 for * or _

        return TextEdit {
            range: node_range,
            new_text: inner_text.to_string(),
            new_selection: (range.start - 1)..(range.end - 1),
        };
    }

    // 2. Wrap logic
    let marker = if kind_to_toggle == SyntaxKind::Strong {
        "*"
    } else {
        "_"
    };
    let selected_text = &content[range.clone()];
    let new_text = format!("{}{}{}", marker, selected_text, marker);

    TextEdit {
        range: range.clone(),
        new_text,
        new_selection: (range.start + 1)..(range.end + 1),
    }
}

/// Helper function to find the inner content range of a trailing content block of a FuncCall.
fn get_content_body_range(content: &str, func_call: &LinkedNode) -> Option<Range<usize>> {
    // Search both directly under FuncCall and under its Args child (Typst 0.14 layout)
    let content_node = func_call
        .children()
        .find(|child| child.kind() == SyntaxKind::ContentBlock)
        .or_else(|| {
            func_call
                .children()
                .find(|child| child.kind() == SyntaxKind::Args)
                .and_then(|args| {
                    args.children()
                        .find(|child| child.kind() == SyntaxKind::ContentBlock)
                })
        })?;

    let r = content_node.range();
    if r.len() >= 2 {
        Some((r.start + 1)..(r.end - 1))
    } else {
        None
    }
}

/// Recursively find all text FuncCalls in the tree.
fn collect_text_nodes<'a>(node: &LinkedNode<'a>, nodes: &mut Vec<LinkedNode<'a>>) {
    if node.kind() == SyntaxKind::FuncCall {
        if let Some(callee) = node.children().next() {
            //should full_text or leaf_text be used here
            let callee_text = callee.full_text();
            if callee_text == "text" || callee_text == "#text" {
                nodes.push(node.clone());
            }
        }
    }
    for child in node.children() {
        collect_text_nodes(&child, nodes);
    }
}

/// Find if there's a valid `#text` node we can merge arguments into instead of nesting.
fn find_target_text_node_for_merge<'a>(
    content: &str,
    root: &'a LinkedNode<'a>,
    range: Range<usize>,
) -> Option<(LinkedNode<'a>, Range<usize>)> {
    let mut candidates = Vec::new();
    collect_text_nodes(root, &mut candidates);

    // Innermost first
    for node in candidates.iter().rev() {
        let node_range = node.range();
        if let Some(body_range) = get_content_body_range(content, node) {
            // Case 1: Selection is inside/covers content body
            if range.start >= body_range.start && range.end <= body_range.end {
                if range.is_empty()
                    || (range.start == body_range.start && range.end == body_range.end)
                {
                    return Some((node.clone(), body_range));
                }
            }
            // Case 2: Selection matches the entire FuncCall itself
            if range.start == node_range.start && range.end == node_range.end {
                return Some((node.clone(), body_range));
            }
        }
    }

    None
}

/// Safely modifies content while dynamically shifting selection coordinates
fn adjust_selection(
    replace_range: Range<usize>,
    replacement_len: usize,
    selection: Range<usize>,
) -> Range<usize> {
    let diff = (replacement_len as isize) - (replace_range.len() as isize);

    let mut new_start = selection.start;
    if replace_range.end <= selection.start {
        new_start = (selection.start as isize + diff) as usize;
    } else if replace_range.start < selection.start {
        new_start = replace_range.start + replacement_len;
    }

    let mut new_end = selection.end;
    if replace_range.end <= selection.end {
        new_end = (selection.end as isize + diff) as usize;
    } else if replace_range.start < selection.end {
        new_end = (selection.end as isize + diff) as usize;
    }

    new_start..new_end
}

/// Parses arguments to find if a key is present and returns its value
fn get_arg_value(inner_args: &str, key: &str) -> Option<String> {
    for param in inner_args.split(',') {
        let trimmed = param.trim();
        if let Some((p_key, p_val)) = trimmed.split_once(':') {
            if p_key.trim() == key {
                return Some(p_val.trim().to_string());
            }
        }
    }
    None
}

/// Removes a key from the arguments, returning the new inner args string and whether it is now empty.
fn remove_arg(inner_args: &str, key: &str) -> (String, bool) {
    let mut parts = Vec::new();
    for param in inner_args.split(',') {
        let trimmed = param.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((p_key, _)) = trimmed.split_once(':') {
            if p_key.trim() != key {
                parts.push(trimmed);
            }
        } else {
            parts.push(trimmed);
        }
    }
    let updated = parts.join(", ");
    let is_empty = updated.is_empty();
    (updated, is_empty)
}

/// Intelligently sets parameters in a `#text(...)` block using the AST.
/// Intelligently sets parameters in a `#text(...)` block using the AST.
fn apply_text_param_ast(content: &str, range: Range<usize>, key: &str, value: &str) -> TextEdit {
    // 1. Trim leading/trailing whitespace from the active range to prevent trapping spacing
    let mut trimmed_start = range.start;
    let mut trimmed_end = range.end;
    while trimmed_start < trimmed_end
        && content
            .chars()
            .nth(trimmed_start)
            .map_or(false, |c| c.is_whitespace())
    {
        trimmed_start += 1;
    }
    while trimmed_end > trimmed_start
        && content
            .chars()
            .nth(trimmed_end - 1)
            .map_or(false, |c| c.is_whitespace())
    {
        trimmed_end -= 1;
    }
    let active_range = trimmed_start..trimmed_end;

    let tree = parse(content);
    let root = LinkedNode::new(&tree);

    if let Some((formatting_node, body_range)) =
        find_target_text_node_for_merge(content, &root, active_range.clone())
    {
        if let Some(args_node) = formatting_node
            .children()
            .find(|child| child.kind() == SyntaxKind::Args)
        {
            // Find LeftParen and RightParen inside Args node to locate the parenthesized arguments list
            let left_paren = args_node
                .children()
                .find(|c| c.kind() == SyntaxKind::LeftParen);
            let right_paren = args_node
                .children()
                .find(|c| c.kind() == SyntaxKind::RightParen);

            let (args_range, inner_args) = if let (Some(lp), Some(rp)) = (left_paren, right_paren) {
                let range = lp.range().start..rp.range().end;
                let text = &content[range.clone()];
                (range, &text[1..text.len() - 1])
            } else {
                let range = args_node.range();
                let text = &content[range.clone()];
                let inner = if text.len() >= 2 {
                    &text[1..text.len() - 1]
                } else {
                    ""
                };
                (range, inner)
            };

            // --- SMART UNWRAP / PRUNE REDUNDANT INHERITED ARGUMENTS ---
            if value.is_empty() {
                let (updated_inner, is_empty) = remove_arg(inner_args, key);
                if is_empty {
                    // If no other arguments are left, unwrap this #text block completely
                    let inner_body_text = &content[body_range.clone()];
                    let new_selection = adjust_selection(
                        formatting_node.range(),
                        inner_body_text.len(),
                        range.clone(),
                    );
                    return TextEdit {
                        range: formatting_node.range(),
                        new_text: inner_body_text.to_string(),
                        new_selection,
                    };
                } else {
                    // Just remove the redundant property from args list
                    let updated_args = format!("({})", updated_inner);
                    let new_selection =
                        adjust_selection(args_range.clone(), updated_args.len(), range.clone());
                    return TextEdit {
                        range: args_range,
                        new_text: updated_args,
                        new_selection,
                    };
                }
            }

            // Standard property merge
            let updated_inner = merge_args(inner_args, key, value);
            let updated_args = format!("({})", updated_inner);
            let new_selection =
                adjust_selection(args_range.clone(), updated_args.len(), range.clone());
            return TextEdit {
                range: args_range,
                new_text: updated_args,
                new_selection,
            };
        } else {
            // No Args node found, insert it right after the callee (first child of FuncCall)
            if let Some(callee_node) = formatting_node.children().next() {
                let insert_pos = callee_node.range().end;
                let inserted_str = format!("({}: {})", key, value);
                let new_selection =
                    adjust_selection(insert_pos..insert_pos, inserted_str.len(), range.clone());
                return TextEdit {
                    range: insert_pos..insert_pos,
                    new_text: inserted_str,
                    new_selection,
                };
            }
        }
    }

    // Default Case: Wrap range with a brand new text function call (nested)
    let prefix = format!("#text({}: {})[", key, value);
    let reconstructed = format!("{}{}]", prefix, &content[active_range.clone()]);
    let new_start = active_range.start + prefix.len();
    let new_range = new_start..(new_start + active_range.len());
    TextEdit {
        range: active_range.clone(),
        new_text: reconstructed,
        new_selection: new_range,
    }
}

/// Helper function to merge arguments in a function block
fn merge_args(args_block: &str, key: &str, value: &str) -> String {
    let mut params: Vec<String> = args_block
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let mut key_replaced = false;
    for param in &mut params {
        if let Some((p_key, _)) = param.split_once(':') {
            if p_key.trim() == key {
                *param = format!("{}: {}", key, value);
                key_replaced = true;
            }
        }
    }

    if !key_replaced {
        params.push(format!("{}: {}", key, value));
    }

    params.join(", ")
}

/// Helper function to find standard `#set page(...)` rules in the syntax tree
fn find_page_set_rule<'a>(root: &'a LinkedNode<'a>) -> Option<LinkedNode<'a>> {
    let mut stack = vec![root.clone()];
    while let Some(node) = stack.pop() {
        if node.kind() == SyntaxKind::SetRule {
            if let Some(target) = node
                .children()
                .find(|child| child.kind() == SyntaxKind::Ident)
            {
                //should full_text or leaf_text be used here
                if target.full_text() == "page" {
                    return Some(node);
                }
            }
        }
        for child in node.children().rev() {
            stack.push(child);
        }
    }
    None
}

/// Safely searches for `#set page(...)` at the top of the file, updating an active attribute,
/// or prepending a new page set rule if none exists using AST syntax parsing.
fn update_or_insert_page_rule_ast(content: &str, key: &str, value: &str) -> (Range<usize>, String) {
    let tree = parse(content);
    let root = LinkedNode::new(&tree);

    if let Some(set_rule_node) = find_page_set_rule(&root) {
        if let Some(args_node) = set_rule_node
            .children()
            .find(|child| child.kind() == SyntaxKind::Args)
        {
            let args_range = args_node.range();
            let args_text = &content[args_range.clone()];
            let inner_args = if args_text.len() >= 2 {
                &args_text[1..args_text.len() - 1]
            } else {
                ""
            };
            let updated_inner = merge_args(inner_args, key, value);
            let updated_args = format!("({})", updated_inner);

            return (args_range, updated_args);
        } else {
            // Target found but no Args node (e.g., #set page)
            let insert_pos = set_rule_node.range().end;
            let inserted_str = format!("({}: {})", key, value);
            return (insert_pos..insert_pos, inserted_str);
        }
    }

    // No existing #set page directive found: Prepend to the top of the document
    (0..0, format!("#set page({}: {})\n", key, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toggle_formatting() {
        let content = "Hello world";

        // 1. Bold the word "world"
        let edit = apply_edit_action(content, 6..11, &EditAction::ToggleBold);
        assert_eq!(edit.new_text, "*world*");
        assert_eq!(edit.range, 6..11);

        let new_content = format!(
            "{}{}{}",
            &content[..edit.range.start],
            edit.new_text,
            &content[edit.range.end..]
        );
        assert_eq!(new_content, "Hello *world*");

        // 2. Unbold it back
        let edit_unwrap = apply_edit_action(&new_content, 7..12, &EditAction::ToggleBold);
        assert_eq!(edit_unwrap.new_text, "world");
        assert_eq!(edit_unwrap.range, 6..13);
    }

    #[test]
    fn test_smart_nesting_prevention() {
        let content = "Hello #text(font: \"Inter\")[world]";

        // Target is "world" which is inside #text(...)
        // Let's modify size of "world" (selection inside the body)
        let edit = apply_edit_action(content, 27..32, &EditAction::SetFontSize(14.0));

        // It must merge the size parameter rather than wrapping it in `#text(size: 14pt)[...]`
        assert_eq!(edit.new_text, "(font: \"Inter\", size: 14pt)");
        // The edit range must target only the Args node of the parent '#text(...)`
        assert_eq!(edit.range, 11..26);
    }

    #[test]
    fn test_page_set_rules() {
        let content = "Hello reader";

        // 1. Insert paper attribute on a completely empty set-rule space
        let edit = apply_edit_action(content, 0..0, &EditAction::SetPaper("A4".to_string()));
        assert_eq!(edit.new_text, "#set page(paper: \"A4\")\n");
        assert_eq!(edit.range, 0..0);

        // 2. Update existing set rule
        let content_with_page = "#set page(paper: \"A4\")\nHello reader";
        let edit_update = apply_edit_action(
            content_with_page,
            24..30, // reader
            &EditAction::SetFlipped(true),
        );
        assert_eq!(edit_update.new_text, "(paper: \"A4\", flipped: true)");
        assert_eq!(edit_update.range, 9..22);
    }
}
