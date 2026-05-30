// crates/typforge/src/ribbon/injector.rs

use crate::actions::RibbonAction;
use std::ops::Range;
use typst::syntax::{LinkedNode, Side, SyntaxKind, parse};

pub struct TextEdit {
    pub range: Range<usize>,
    pub new_text: String,
    pub new_selection: Range<usize>,
}

/// Applies a RibbonAction to the source text.
/// Returns the localized TextEdit to apply.
pub fn apply_ribbon_action(
    content: &str,
    selection: Range<usize>,
    action: &RibbonAction,
) -> TextEdit {
    let start = selection.start.min(content.len());
    let end = selection.end.min(content.len());
    let selection_clamped = start..end;

    match action {
        // --- 1. TEXT PARAMETERS (Smart Toggles & Mergers) ---
        RibbonAction::ToggleBold => {
            toggle_wrapper_ast(content, selection_clamped, SyntaxKind::Strong)
        }
        RibbonAction::ToggleItalic => {
            toggle_wrapper_ast(content, selection_clamped, SyntaxKind::Emph)
        }
        RibbonAction::SetFont(font_family) => apply_text_param_ast(
            content,
            selection_clamped,
            "font",
            &format!("\"{}\"", font_family),
        ),
        RibbonAction::SetFontSize(size) => {
            apply_text_param_ast(content, selection_clamped, "size", &format!("{}pt", size))
        }
        RibbonAction::SetTextColor(color) => {
            apply_text_param_ast(content, selection_clamped, "fill", color)
        }

        // --- 2. COMPLEX ELEMENTS (Cursor Injections) ---
        RibbonAction::InsertGrid { rows, cols } => {
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
        RibbonAction::SetPaper(paper) => {
            let (edit_range, edit_text) =
                update_or_insert_page_rule_ast(content, "paper", &format!("\"{}\"", paper));
            let new_selection = adjust_selection(edit_range.clone(), edit_text.len(), selection);
            TextEdit {
                range: edit_range,
                new_text: edit_text,
                new_selection,
            }
        }
        RibbonAction::SetFlipped(flipped) => {
            let (edit_range, edit_text) =
                update_or_insert_page_rule_ast(content, "flipped", &flipped.to_string());
            let new_selection = adjust_selection(edit_range.clone(), edit_text.len(), selection);
            TextEdit {
                range: edit_range,
                new_text: edit_text,
                new_selection,
            }
        }
        RibbonAction::SetColumns(cols) => {
            let (edit_range, edit_text) =
                update_or_insert_page_rule_ast(content, "columns", &cols.to_string());
            let new_selection = adjust_selection(edit_range.clone(), edit_text.len(), selection);
            TextEdit {
                range: edit_range,
                new_text: edit_text,
                new_selection,
            }
        }
        RibbonAction::SetMargin(margin) => {
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
    let leaf = root
        .leaf_at(range.start, Side::Before)
        .or_else(|| root.leaf_at(range.start, Side::After))?;
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

/// Recursively find all formatting nodes that overlap or touch the target range
fn find_intersecting_formatting_nodes<'a>(
    root: &LinkedNode<'a>,
    range: Range<usize>,
    kind: SyntaxKind,
    nodes: &mut Vec<LinkedNode<'a>>,
) {
    if root.kind() == kind {
        let node_range = root.range();
        if node_range.start <= range.end && node_range.end >= range.start {
            nodes.push(root.clone());
            return;
        }
    }
    for child in root.children() {
        find_intersecting_formatting_nodes(&child, range.clone(), kind, nodes);
    }
}

/// Helper to map original selection indexes to the new formatted string layout
fn map_index(
    orig_idx: usize,
    combined_start: usize,
    marker_ranges: &[Range<usize>],
    marker_len: usize,
) -> usize {
    let mut markers_removed_before = 0;
    for r in marker_ranges {
        if r.end <= orig_idx {
            markers_removed_before += r.len();
        } else if r.start < orig_idx {
            markers_removed_before += orig_idx - r.start;
        }
    }
    combined_start + marker_len + (orig_idx - combined_start - markers_removed_before)
}

/// Toggles bold or italic using synchronous syntax trees.
fn toggle_wrapper_ast(content: &str, range: Range<usize>, kind_to_toggle: SyntaxKind) -> TextEdit {
    let tree = parse(content);
    let root = LinkedNode::new(&tree);
    let marker = if kind_to_toggle == SyntaxKind::Strong {
        "*"
    } else {
        "_"
    };
    let marker_len = marker.len();

    // 1. Check if the selection is already wrapped
    if let Some(node) = find_formatting_node(&root, range.clone(), kind_to_toggle) {
        let node_range = node.range();

        // Ensure we don't underflow
        if node_range.start + marker_len <= node_range.end - marker_len {
            let inner_range = (node_range.start + marker_len)..(node_range.end - marker_len);
            let inner_text = &content[inner_range];

            return TextEdit {
                range: node_range,
                new_text: inner_text.to_string(),
                // Use saturating sub or check for valid range to avoid overflow
                new_selection: (range.start.saturating_sub(marker_len))
                    ..(range.end.saturating_sub(marker_len)),
            };
        }
    }

    // 2. Wrap logic
    let selected_text = &content[range.clone()];
    let new_text = format!("{}{}{}", marker, selected_text, marker);

    TextEdit {
        range: range.clone(),
        new_text,
        new_selection: (range.start + marker_len)..(range.end + marker_len),
    }
}

/// Helper function to find the inner content range of a trailing content block of a FuncCall.
fn get_content_body_range(func_call: &LinkedNode) -> Option<Range<usize>> {
    let content_node = func_call
        .children()
        .find(|child| child.kind() == SyntaxKind::ContentBlock)?;
    let r = content_node.range();
    if r.len() >= 2 {
        Some((r.start + 1)..(r.end - 1))
    } else {
        None
    }
}

/// Recursively finds all `#text` or `text` function call nodes that fully enclose the selection range,
/// ordered from outermost to innermost.
fn find_enclosing_text_nodes<'a>(
    root: &LinkedNode<'a>,
    range: Range<usize>,
    nodes: &mut Vec<LinkedNode<'a>>,
) {
    if root.kind() == SyntaxKind::FuncCall {
        if let Some(callee) = root.children().next() {
            let callee_text = callee.text();
            if callee_text == "text" || callee_text == "#text" {
                if let Some(body_range) = get_content_body_range(root) {
                    if range.start >= body_range.start && range.end <= body_range.end {
                        nodes.push(root.clone());
                    }
                }
            }
        }
    }
    for child in root.children() {
        find_enclosing_text_nodes(&child, range.clone(), nodes);
    }
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

    // 2. Locate all nesting text ancestors covering this active span
    let mut enclosing_nodes = Vec::new();
    find_enclosing_text_nodes(&root, active_range.clone(), &mut enclosing_nodes);

    if let Some(formatting_node) = enclosing_nodes.last().cloned() {
        let mut should_merge = false;
        if let Some(body_range) = get_content_body_range(&formatting_node) {
            // Merge arguments if selection is empty or completely fills the block's text body
            if active_range.is_empty()
                || (active_range.start <= body_range.start && active_range.end >= body_range.end)
            {
                should_merge = true;
            }
        }

        if should_merge {
            // Find parent inherited value for this key going up the hierarchy
            let mut parent_value = None;
            if enclosing_nodes.len() > 1 {
                let parent_node = &enclosing_nodes[enclosing_nodes.len() - 2];
                if let Some(parent_args) = parent_node
                    .children()
                    .find(|c| c.kind() == SyntaxKind::Args)
                {
                    let parent_args_text = parent_args.text();
                    let parent_inner = if parent_args_text.len() >= 2 {
                        &parent_args_text[1..parent_args_text.len() - 1]
                    } else {
                        ""
                    };
                    parent_value = get_arg_value(parent_inner, key);
                }
            }

            if let Some(args_node) = formatting_node
                .children()
                .find(|child| child.kind() == SyntaxKind::Args)
            {
                let args_range = args_node.range();
                let args_text = args_node.text();
                let inner_args = if args_text.len() >= 2 {
                    &args_text[1..args_text.len() - 1]
                } else {
                    ""
                };

                // --- SMART UNWRAP / PRUNE REDUNDANT INHERITED ARGUMENTS ---
                if value.is_empty() || parent_value.as_deref() == Some(value) {
                    let (updated_inner, is_empty) = remove_arg(inner_args, key);
                    if is_empty {
                        // If no other arguments are left, unwrap this #text block completely
                        if let Some(body_range) = get_content_body_range(&formatting_node) {
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
                        }
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
    let mut updated = String::new();
    let mut key_replaced = false;

    for param in args_block.split(',') {
        let trimmed = param.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some((p_key, _)) = trimmed.split_once(':') {
            if p_key.trim() == key {
                if !updated.is_empty() {
                    updated.push_str(", ");
                }
                updated.push_str(&format!("{}: {}", key, value));
                key_replaced = true;
            } else {
                if !updated.is_empty() {
                    updated.push_str(", ");
                }
                updated.push_str(trimmed);
            }
        } else {
            if !updated.is_empty() {
                updated.push_str(", ");
            }
            updated.push_str(trimmed);
        }
    }

    if !key_replaced {
        if !updated.is_empty() {
            updated.push_str(", ");
        }
        updated.push_str(&format!("{}: {}", key, value));
    }

    updated
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
                if target.text() == "page" {
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
            let args_text = args_node.text();
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
