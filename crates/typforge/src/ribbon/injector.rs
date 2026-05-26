// crates/typforge/src/ribbon/injector.rs

use crate::actions::RibbonAction;
use std::ops::Range;

/// Applies a RibbonAction to the source text.
/// Returns the mutated document string and the adjusted selection range.
pub fn apply_ribbon_action(
    content: &str,
    selection: Range<usize>,
    action: &RibbonAction,
) -> (String, Range<usize>) {
    let start = selection.start.min(content.len());
    let end = selection.end.min(content.len());
    let selected_text = &content[start..end];

    match action {
        // --- 1. TEXT PARAMETERS (Smart Toggles & Mergers) ---
        RibbonAction::ToggleBold => toggle_wrapper(content, start, end, selected_text, "*", "*"),
        RibbonAction::ToggleItalic => toggle_wrapper(content, start, end, selected_text, "_", "_"),
        RibbonAction::SetFont(font_family) => apply_text_param(
            content,
            start,
            end,
            selected_text,
            "font",
            &format!("\"{}\"", font_family),
        ),
        RibbonAction::SetFontSize(size) => apply_text_param(
            content,
            start,
            end,
            selected_text,
            "size",
            &format!("{}pt", size),
        ),
        RibbonAction::SetTextColor(color) => {
            apply_text_param(content, start, end, selected_text, "fill", color)
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

            let mut new_content = content.to_string();
            new_content.insert_str(start, &grid_markup);

            // Restore selection to cover the entire grid block
            (new_content, start..(start + grid_markup.len()))
        }

        // --- 3. PAGE PARAMETERS (Global Directive Setters) ---
        RibbonAction::SetPaper(paper) => {
            let updated = update_or_insert_page_rule(content, "paper", &format!("\"{}\"", paper));
            (updated, selection)
        }
        RibbonAction::SetFlipped(flipped) => {
            let updated = update_or_insert_page_rule(content, "flipped", &flipped.to_string());
            (updated, selection)
        }
        RibbonAction::SetColumns(cols) => {
            let updated = update_or_insert_page_rule(content, "columns", &cols.to_string());
            (updated, selection)
        }
        RibbonAction::SetMargin(margin) => {
            let updated = update_or_insert_page_rule(content, "margin", &format!("({})", margin));
            (updated, selection)
        }
    }
}

/// Wraps text inside a prefix and suffix, adjusting selection coordinates safely.
fn wrap_text(
    content: &str,
    start: usize,
    end: usize,
    selected_text: &str,
    prefix: &str,
    suffix: &str,
) -> (String, Range<usize>) {
    let mut new_content = content[..start].to_string();
    new_content.push_str(prefix);
    new_content.push_str(selected_text);
    new_content.push_str(suffix);
    new_content.push_str(&content[end..]);

    let new_start = start + prefix.len();
    let new_end = new_start + selected_text.len();
    (new_content, new_start..new_end)
}

/// Wraps selected text, or toggles (unwraps) it if already wrapped (inside OR outside the selection bounds).
fn toggle_wrapper(
    content: &str,
    start: usize,
    end: usize,
    selected_text: &str,
    prefix: &str,
    suffix: &str,
) -> (String, Range<usize>) {
    let p_len = prefix.len();

    // --- State Check: Are we inside an existing bold/italic block? ---
    let outer_prefix_idx = content[..start].rfind(prefix);
    let outer_suffix_idx = content[end..].find(suffix).map(|idx| end + idx);

    let is_inside_block = if let (Some(pre), Some(suf)) = (outer_prefix_idx, outer_suffix_idx) {
        // Verify there isn't a closing suffix between the prefix and our selection start
        !content[pre + p_len..start].contains(suffix) && !content[end..suf].contains(prefix)
    } else {
        false
    };

    if is_inside_block {
        // --- OPERATION: UNBOLD (Remove formatting from selection) ---
        let pre_idx = outer_prefix_idx.unwrap();
        let suf_idx = outer_suffix_idx.unwrap();

        let before_text = &content[pre_idx + p_len..start];
        let after_text = &content[end..suf_idx];

        let mut reconstructed = String::new();

        // Re-bold the left segment if it has content
        if !before_text.is_empty() {
            reconstructed.push_str(prefix);
            reconstructed.push_str(before_text);
            reconstructed.push_str(suffix);
        }

        // Leave the selected text completely unbolded (strip any stars inside it just in case)
        let cleaned_selection = selected_text.replace(prefix, "");
        reconstructed.push_str(&cleaned_selection);

        // Re-bold the right segment if it has content
        if !after_text.is_empty() {
            reconstructed.push_str(prefix);
            reconstructed.push_str(after_text);
            reconstructed.push_str(suffix);
        }

        // Replace the entire outer block with our split/reconstructed version
        let block_end = suf_idx + suffix.len();
        let shift_start = pre_idx
            + if before_text.is_empty() {
                0
            } else {
                p_len + before_text.len() + suffix.len()
            };
        let new_range = shift_start..(shift_start + cleaned_selection.len());

        let mut new_content = content[..pre_idx].to_string();
        new_content.push_str(&reconstructed);
        new_content.push_str(&content[block_end..]);

        return (new_content, new_range);
    }

    // --- OPERATION: BOLD (Apply formatting, merge with adjacent blocks) ---
    // Check if the selection borders existing bold markers immediately
    let has_prefix_left = start >= p_len && &content[start - p_len..start] == prefix;
    let has_suffix_right = end + p_len <= content.len() && &content[end..end + p_len] == suffix;

    if has_prefix_left && has_suffix_right {
        // Double-wrapped boundary edge case: e.g. *selection* -> strip outer stars
        let unwrapped = selected_text.replace(prefix, "");
        let mut new_content = content[..start - p_len].to_string();
        new_content.push_str(&unwrapped);
        new_content.push_str(&content[end + p_len..]);
        return (
            new_content,
            (start - p_len)..(start - p_len + unwrapped.len()),
        );
    }

    // Check if selection overlaps/touches a bold block on the right, e.g. "regular *text*"
    if end + p_len <= content.len() && &content[end..end + p_len] == prefix {
        // Merge selection into right block: "regular *text*" -> "*regular text*"
        if let Some(right_suffix_idx) = content[end + p_len..]
            .find(suffix)
            .map(|idx| end + p_len + idx)
        {
            let right_inner = &content[end + p_len..right_suffix_idx];
            let merged_block = format!("{}{}{}{}", prefix, selected_text, right_inner, suffix);

            let mut new_content = content[..start].to_string();
            new_content.push_str(&merged_block);
            new_content.push_str(&content[right_suffix_idx + p_len..]);

            let new_range = (start + p_len)..(start + p_len + selected_text.len());
            return (new_content, new_range);
        }
    }

    // Check if selection overlaps/touches a bold block on the left, e.g. "*text* regular"
    if start >= p_len && &content[start - p_len..start] == suffix {
        // Merge selection into left block: "*text* regular" -> "*text regular*"
        if let Some(left_prefix_idx) = content[..start - p_len].rfind(prefix) {
            let left_inner = &content[left_prefix_idx + p_len..start - p_len];
            let merged_block = format!("{}{}{}{}", prefix, left_inner, selected_text, suffix);

            let mut new_content = content[..left_prefix_idx].to_string();
            new_content.push_str(&merged_block);
            new_content.push_str(&content[end..]);

            let new_start = left_prefix_idx + p_len + left_inner.len();
            let new_range = new_start..(new_start + selected_text.len());
            return (new_content, new_range);
        }
    }

    // Default Case: Simple wrap
    wrap_text(content, start, end, selected_text, prefix, suffix)
}

/// Finds if the selection is enclosed by the prefix/suffix
fn find_outer_boundary(
    content: &str,
    start: usize,
    end: usize,
    prefix: &str,
) -> (usize, usize, bool) {
    let prev_start = content[..start].rfind(prefix);
    let next_end = content[end..].find(prefix);

    if let (Some(s), Some(e)) = (prev_start, next_end) {
        (s, end + e + prefix.len(), true)
    } else {
        (start, end, false)
    }
}

fn replace_range(
    content: &str,
    start: usize,
    end: usize,
    new_text: &str,
) -> (String, Range<usize>) {
    let mut new_content = content[..start].to_string();
    new_content.push_str(new_text);
    new_content.push_str(&content[end..]);
    (new_content, start..(start + new_text.len()))
}

/// Intelligently sets parameters in a `#text(...)` block.
/// If wrapped already, it mutates the existing `#text` block rather than double-nesting.
fn apply_text_param(
    content: &str,
    start: usize,
    end: usize,
    selected_text: &str,
    key: &str,
    value: &str,
) -> (String, Range<usize>) {
    // Check if selection is wrapped in `#text(...)[...]`
    if selected_text.starts_with("#text(") && selected_text.ends_with("]") {
        if let Some(bracket_idx) = selected_text.find('[') {
            let args_block = &selected_text[6..bracket_idx - 1]; // inside #text(...)
            let inner_text = &selected_text[bracket_idx + 1..selected_text.len() - 1]; // inside [...]

            let updated_args = merge_args(args_block, key, value);
            let updated_wrapper = format!("#text({})[{}]", updated_args, inner_text);

            let mut new_content = content[..start].to_string();
            new_content.push_str(&updated_wrapper);
            new_content.push_str(&content[end..]);

            return (new_content, start..(start + updated_wrapper.len()));
        }
    }

    // Check if wrapping borders exist immediately outside the selection
    if start >= 6 && end + 1 <= content.len() {
        let has_suffix_outside = &content[end..end + 1] == "]";
        let prefix_segment = &content[..start];
        if let Some(hash_idx) = prefix_segment.rfind("#text(") {
            if prefix_segment[hash_idx..].ends_with('[') {
                let args_block = &prefix_segment[hash_idx + 6..start - 2]; // Extract arguments
                let updated_args = merge_args(args_block, key, value);

                let mut new_content = content[..hash_idx].to_string();
                new_content.push_str(&format!("#text({})[{}", updated_args, selected_text));
                new_content.push_str(&content[end..]);

                let offset_shift = (hash_idx + 7 + updated_args.len()) as isize - start as isize;
                let new_start = (start as isize + offset_shift) as usize;
                let new_end = new_start + selected_text.len();

                return (new_content, new_start..new_end);
            }
        }
    }

    // Base Case: Create a clean new `#text(...)` wrapper
    let prefix = format!("#text({}: {})[", key, value);
    wrap_text(content, start, end, selected_text, &prefix, "]")
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

/// Safely searches for `#set page(...)` at the top of the file, updating an active attribute,
/// or prepending a new page set rule at line 1 if none exists.
fn update_or_insert_page_rule(content: &str, key: &str, value: &str) -> String {
    let target = "#set page(";

    if let Some(start_idx) = content.find(target) {
        let args_start = start_idx + target.len();

        // Find matching closing parenthesis
        if let Some(end_offset) = find_closing_parenthesis(&content[args_start..]) {
            let end_idx = args_start + end_offset;
            let args_block = &content[args_start..end_idx];

            let updated_args = merge_args(args_block, key, value);

            let mut new_content = content[..start_idx].to_string();
            new_content.push_str(&format!("#set page({})", updated_args));
            new_content.push_str(&content[end_idx + 1..]);
            return new_content;
        }
    }

    // No existing #set page directive found: Prepend to the top of the document
    let mut new_content = format!("#set page({}: {})\n", key, value);
    new_content.push_str(content);
    new_content
}

/// Helper to scan bracket-matching to locate the closing parenthesis of a directive.
fn find_closing_parenthesis(slice: &str) -> Option<usize> {
    let mut depth = 1;
    for (idx, ch) in slice.char_indices() {
        if ch == '(' {
            depth += 1;
        } else if ch == ')' {
            depth -= 1;
            if depth == 0 {
                return Some(idx);
            }
        }
    }
    None
}
