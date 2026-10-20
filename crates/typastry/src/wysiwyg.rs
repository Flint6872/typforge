use std::collections::HashMap;
use typst_syntax::{LinkedNode, Side, Source, SyntaxKind, ast, ast::AstNode};

// ==========================================
// PILLAR 1: AST MUTATION ENGINE (DISCRETE VISUAL EDITS)
// ==========================================

/// Safe transactional mutation representation.
pub struct MutationTransaction {
    pub original_text: String,
    pub original_cursor: usize,
    pub original_selection: Option<std::ops::Range<usize>>,
}

/// Applies a named property/argument change to a function call in the source text.
/// Uses defensive AST lookups and performs safe string-slice mutation.
pub fn apply_named_argument_edit(
    source_text: &str,
    element_offset: usize,
    property_name: &str,
    property_value: &str,
) -> Option<(String, std::ops::Range<usize>)> {
    let source = Source::detached(source_text);
    let root = LinkedNode::new(source.root());
    let leaf = root.leaf_at(element_offset, Side::Before)?;

    // Walk up ancestors to find the target FuncCall
    let mut current = leaf;
    let mut func_call_node = None;
    loop {
        if current.kind() == SyntaxKind::FuncCall {
            func_call_node = Some(current);
            break;
        }
        if let Some(parent) = current.parent() {
            current = parent.clone();
        } else {
            break;
        }
    }

    let func_node = func_call_node?;

    // Find the Args child node
    let mut args_node = None;
    for child in func_node.children() {
        if child.kind() == SyntaxKind::Args {
            args_node = Some(child);
            break;
        }
    }

    let args = args_node?;

    // Check if the named argument already exists in the call
    let mut existing_named_node = None;
    for child in args.children() {
        if child.kind() == SyntaxKind::Named {
            if let Some(named_ast) = child.cast::<ast::Named>() {
                if named_ast.name().as_str() == property_name {
                    existing_named_node = Some(child);
                    break;
                }
            }
        }
    }

    if let Some(named_node) = existing_named_node {
        // Find the child node representing the value expression of the named argument
        let mut value_node = None;
        if let Some(named_ast) = named_node.cast::<ast::Named>() {
            let expr = named_ast.expr();
            for c in named_node.children() {
                if c.span() == expr.span() {
                    value_node = Some(c);
                    break;
                }
            }
        }

        let target_node = match value_node {
            Some(node) => node,
            None => named_node.clone(),
        };

        let range = target_node.range();
        let mut new_text = source_text.to_string();
        new_text.replace_range(range.clone(), property_value);

        // Calculate new value range in the mutated source string
        let diff_len = property_value.len() as isize - (range.end - range.start) as isize;
        let new_range = range.start..(range.end as isize + diff_len) as usize;
        Some((new_text, new_range))
    } else {
        // Named argument does not exist; append it safely
        let args_range = args.range();
        let args_text = &source_text[args_range.clone()];

        if let Some(closing_paren_idx) = args_text.rfind(')') {
            let insert_offset = args_range.start + closing_paren_idx;

            // Check if we need a leading comma separator
            let has_other_args = args
                .cast::<ast::Args>()
                .map(|a| a.items().next().is_some())
                .unwrap_or(false);

            let insertion = if has_other_args {
                format!(", {}: {}", property_name, property_value)
            } else {
                format!("{}: {}", property_name, property_value)
            };

            let mut new_text = source_text.to_string();
            new_text.insert_str(insert_offset, &insertion);

            let start = insert_offset + insertion.len() - property_value.len();
            let end = start + property_value.len();
            Some((new_text, start..end))
        } else {
            None
        }
    }
}

// ==========================================
// PILLAR 2: SYNTAX-AWARE CURSOR TRAVERSAL
// ==========================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorDirection {
    Left,
    Right,
}

pub enum ContentBoundary {
    LeftBracket(std::ops::Range<usize>),
    RightBracket(std::ops::Range<usize>),
}

pub struct CursorNavigator;

impl CursorNavigator {
    /// Computes the next valid visual index in the given direction.
    /// Skips non-visual structural nodes such as brackets, parentheses, keywords, and commas.
    pub fn move_cursor(
        source_text: &str,
        current_offset: usize,
        direction: CursorDirection,
    ) -> usize {
        let source = Source::detached(source_text);
        let root = LinkedNode::new(source.root());
        let len = source_text.len();

        let mut target_offset = current_offset;

        match direction {
            CursorDirection::Right => {
                if target_offset >= len {
                    return len;
                }

                target_offset = step_char_forward(source_text, target_offset);

                while target_offset < len {
                    if let Some(node) = root.leaf_at(target_offset, Side::Before) {
                        if Self::is_visual_node(&node) {
                            break;
                        } else {
                            let range = node.range();
                            if range.end > target_offset {
                                target_offset = range.end;
                            } else {
                                target_offset = step_char_forward(source_text, target_offset);
                            }
                        }
                    } else {
                        target_offset = step_char_forward(source_text, target_offset);
                    }
                }
                target_offset.min(len)
            }
            CursorDirection::Left => {
                if target_offset == 0 {
                    return 0;
                }

                target_offset = step_char_backward(source_text, target_offset);

                while target_offset > 0 {
                    if let Some(node) = root.leaf_at(target_offset, Side::Before) {
                        if Self::is_visual_node(&node) {
                            break;
                        } else {
                            let range = node.range();
                            if range.start < target_offset {
                                target_offset = range.start;
                            } else {
                                target_offset = step_char_backward(source_text, target_offset);
                            }
                        }
                    } else {
                        target_offset = step_char_backward(source_text, target_offset);
                    }
                }
                target_offset
            }
        }
    }

    /// Checks if a node qualifies as visual (e.g. Text, Space, or single visual block functions).
    fn is_visual_node(node: &LinkedNode<'_>) -> bool {
        match node.kind() {
            SyntaxKind::Text | SyntaxKind::Space => true,
            SyntaxKind::FuncCall => {
                if let Some(func_call) = node.cast::<ast::FuncCall>() {
                    if let ast::Expr::Ident(ident) = func_call.callee() {
                        matches!(
                            ident.as_str(),
                            "image" | "rect" | "circle" | "line" | "square" | "ellipse"
                        )
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Confines visual edits inside a Content Block boundary if the cursor sits next to content brackets.
    pub fn is_at_content_block_boundary(
        source_text: &str,
        offset: usize,
    ) -> Option<ContentBoundary> {
        let source = Source::detached(source_text);
        let root = LinkedNode::new(source.root());
        let leaf = root.leaf_at(offset, Side::Before)?;

        match leaf.kind() {
            SyntaxKind::LeftBracket => Some(ContentBoundary::LeftBracket(leaf.range())),
            SyntaxKind::RightBracket => Some(ContentBoundary::RightBracket(leaf.range())),
            _ => {
                if let Some(prev) = leaf.prev_sibling() {
                    if prev.kind() == SyntaxKind::LeftBracket {
                        return Some(ContentBoundary::LeftBracket(prev.range()));
                    }
                }
                if let Some(next) = leaf.next_sibling() {
                    if next.kind() == SyntaxKind::RightBracket {
                        return Some(ContentBoundary::RightBracket(next.range()));
                    }
                }
                None
            }
        }
    }
}

fn step_char_forward(s: &str, offset: usize) -> usize {
    let indices = s.char_indices().map(|(idx, _)| idx);
    for idx in indices {
        if idx > offset {
            return idx;
        }
    }
    s.len()
}

fn step_char_backward(s: &str, offset: usize) -> usize {
    let mut prev = 0;
    for (idx, _) in s.char_indices() {
        if idx >= offset {
            return prev;
        }
        prev = idx;
    }
    prev
}

// ==========================================
// PILLAR 3: RIGHT-CLICK INTROSPECTION & CONTEXT MENUS
// ==========================================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ElementProperties {
    pub element_type: String,
    pub element_range: std::ops::Range<usize>,
    pub active_args: HashMap<String, String>,
    pub available_properties: Vec<String>,
}

pub fn get_schema_for_element(element_name: &str) -> &'static [&'static str] {
    match element_name {
        "rect" => &[
            "fill", "stroke", "width", "height", "radius", "inset", "outset",
        ],
        "image" => &["width", "height", "alt", "fit"],
        "text" => &["size", "font", "fill", "weight", "style", "tracking"],
        "circle" => &["radius", "fill", "stroke"],
        "line" => &["length", "angle", "stroke"],
        "square" => &["size", "fill", "stroke", "radius"],
        "ellipse" => &["width", "height", "fill", "stroke"],
        _ => &[],
    }
}

/// Walks up the AST ancestry of the clicked node to find an editable function call,
/// and extracts its parameters alongside available schema fields.
pub fn inspect_element(node: LinkedNode<'_>) -> Option<ElementProperties> {
    let mut current = node;
    let mut func_call_node = None;

    loop {
        if current.kind() == SyntaxKind::FuncCall {
            func_call_node = Some(current);
            break;
        }
        if let Some(parent) = current.parent() {
            current = parent.clone();
        } else {
            break;
        }
    }

    let func_node = func_call_node?;
    let func_call_ast = func_node.cast::<ast::FuncCall>()?;

    let callee_expr = func_call_ast.callee();
    let element_type = match callee_expr {
        ast::Expr::Ident(ident) => ident.as_str().to_string(),
        _ => return None,
    };

    let element_range = func_node.range();
    let mut active_args = HashMap::new();

    // Safely extract existing named arguments from AST
    for child in func_node.children() {
        if child.kind() == SyntaxKind::Args {
            for arg_child in child.children() {
                if arg_child.kind() == SyntaxKind::Named {
                    if let Some(named_ast) = arg_child.cast::<ast::Named>() {
                        let name = named_ast.name().as_str().to_string();
                        let mut value_text = String::new();
                        let mut found_colon = false;
                        for c in arg_child.children() {
                            if found_colon {
                                if c.kind() != SyntaxKind::Space {
                                    value_text = c.full_text().to_string();
                                    break;
                                }
                            } else if c.kind() == SyntaxKind::Colon {
                                found_colon = true;
                            }
                        }
                        active_args.insert(name, value_text);
                    }
                }
            }
        }
    }

    let schema = get_schema_for_element(&element_type);
    let mut available_properties = Vec::new();
    for &prop in schema {
        if !active_args.contains_key(prop) {
            available_properties.push(prop.to_string());
        }
    }

    Some(ElementProperties {
        element_type,
        element_range,
        active_args,
        available_properties,
    })
}
