use crate::actions::RibbonAction;
use typastry::edit::{EditAction, get_arg_value};
use typst::syntax::{LinkedNode, Side, SyntaxKind, parse};

impl From<RibbonAction> for EditAction {
    fn from(action: RibbonAction) -> Self {
        match action {
            RibbonAction::ToggleBold => EditAction::ToggleBold,
            RibbonAction::ToggleItalic => EditAction::ToggleItalic,
            RibbonAction::ToggleUnderline => EditAction::ToggleUnderline,
            RibbonAction::SetFont(f) => EditAction::SetFont(f),
            RibbonAction::SetFontSize(s) => EditAction::SetFontSize(s as f64),
            RibbonAction::SetTextColor(c) => EditAction::SetTextColor(c),
            RibbonAction::InsertGrid { rows, cols } => EditAction::InsertGrid { rows, cols },
            RibbonAction::SetPaper(p) => EditAction::SetPaper(p),
            RibbonAction::SetFlipped(f) => EditAction::SetFlipped(f),
            RibbonAction::SetColumns(c) => EditAction::SetColumns(c),
            RibbonAction::SetMargin(m) => EditAction::SetMargin(m),
        }
    }
}

impl From<&RibbonAction> for EditAction {
    fn from(action: &RibbonAction) -> Self {
        action.clone().into()
    }
}

#[derive(Debug, Clone, Default)]
pub struct ActiveProperties {
    pub is_bold: bool,
    pub is_italic: bool,
    pub is_underline: bool,
    pub size: Option<f32>,
    pub font: Option<String>,
    pub color: Option<String>,
}

/// Traverses up the AST hierarchy from the current cursor position to resolve active properties.
pub fn detect_properties_at_offset(content: &str, offset: usize) -> ActiveProperties {
    let tree = parse(content);
    let root = LinkedNode::new(&tree);
    let mut props = ActiveProperties::default();

    // Locate the leaf node containing the cursor
    let leaf = root
        .leaf_at(offset, Side::Before)
        .or_else(|| root.leaf_at(offset, Side::After));

    let mut current = leaf;
    while let Some(node) = current {
        // 1. Parse Strong/Emph Node Markup
        if node.kind() == SyntaxKind::Strong {
            props.is_bold = true;
        }
        if node.kind() == SyntaxKind::Emph {
            props.is_italic = true;
        }

        // 2. Parse Text Formatting Functions (#text(...) or text(...))
        if node.kind() == SyntaxKind::FuncCall {
            if let Some(callee) = node.children().next() {
                let callee_text = callee.leaf_text();
                if callee_text == "underline" || callee_text == "#underline" {
                    props.is_underline = true;
                }
                if callee_text == "text" || callee_text == "#text" {
                    if let Some(args_node) = node.children().find(|c| c.kind() == SyntaxKind::Args)
                    {
                        let args_text = args_node.leaf_text();
                        let inner_args = if args_text.len() >= 2 {
                            &args_text[1..args_text.len() - 1]
                        } else {
                            ""
                        };

                        // Check weight
                        if let Some(weight_val) = get_arg_value(inner_args, "weight") {
                            let w = weight_val.trim_matches('"');
                            if w == "bold" || w == "700" {
                                props.is_bold = true;
                            }
                        }

                        // Check style
                        if let Some(style_val) = get_arg_value(inner_args, "style") {
                            let s = style_val.trim_matches('"');
                            if s == "italic" {
                                props.is_italic = true;
                            }
                        }

                        // Check size
                        if props.size.is_none() {
                            if let Some(size_val) = get_arg_value(inner_args, "size") {
                                let cleaned = size_val
                                    .trim()
                                    .trim_end_matches("pt")
                                    .trim_end_matches("em");
                                if let Ok(val) = cleaned.parse::<f32>() {
                                    props.size = Some(val);
                                }
                            }
                        }

                        // Check font family
                        if props.font.is_none() {
                            if let Some(font_val) = get_arg_value(inner_args, "font") {
                                props.font = Some(font_val.trim_matches('"').to_string());
                            }
                        }

                        // Check color / fill
                        if props.color.is_none() {
                            if let Some(fill_val) = get_arg_value(inner_args, "fill") {
                                props.color = Some(fill_val);
                            }
                        }
                    }
                }
            }
        }
        current = node.parent().cloned();
    }

    props
}
