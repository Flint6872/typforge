use typst::introspection::PagedPosition;
use typst_syntax::{LinkedNode, Side, Source, SyntaxKind, ast};

#[cfg(feature = "intel")]
use typst::layout::{Abs, Point};
#[cfg(feature = "intel")]
use typst_ide::{IdeWorld, Jump, Tooltip};
#[cfg(feature = "intel")]
use typst_layout::PagedDocument;

/// High-level syntactic context at a cursor byte offset.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum CursorContext {
    InMarkup,
    InCode,
    InMath,
    InFunctionCall {
        name: String,
        supplied_named_args: Vec<String>,
        active_param_index: usize,
    },
    InImportPath(String),
}

/// UI-agnostic jump target returned by `TypastryInspector::jump_from_click`.
#[cfg(feature = "intel")]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum TypastryJump {
    File {
        vpath: String,
        offset: Option<usize>,
    },
    Url(String),
    Position {
        page: usize,
        x_pt: f64,
        y_pt: f64,
    },
}

/// Fast, synchronous AST & IDE inspector for GPUI and GTK frontends.
pub struct TypastryInspector;

impl TypastryInspector {
    /// Inspects `source` at `offset` using `typst-syntax` only (zero-compile, instant).
    pub fn inspect_cursor(source: &Source, offset: usize) -> CursorContext {
        let content = source.text();
        let clamped = offset.min(content.len());
        if content.is_empty() || !content.is_char_boundary(clamped) {
            return CursorContext::InMarkup;
        }

        let root = LinkedNode::new(source.root());
        let Some(leaf) = root
            .leaf_at(clamped, Side::Before)
            .or_else(|| root.leaf_at(clamped, Side::After))
        else {
            return CursorContext::InMarkup;
        };

        // 1. Check for import path string (`#import "@..."`).
        if matches!(leaf.kind(), SyntaxKind::Str | SyntaxKind::Error) {
            let range = leaf.range();
            let text = &content[range.clone()];
            if text.starts_with('"') && clamped > range.start {
                let is_closed =
                    leaf.kind() == SyntaxKind::Str && text.len() >= 2 && text.ends_with('"');
                if !is_closed || clamped < range.end {
                    let mut p = leaf.parent().cloned();
                    while let Some(node) = p {
                        if node.kind() == SyntaxKind::ModuleImport {
                            return CursorContext::InImportPath(
                                content[range.start + 1..clamped].to_string(),
                            );
                        }
                        p = node.parent().cloned();
                    }
                }
            }
        }

        // 2. Helper to classify context starting from a leaf node
        let classify = |start_leaf: LinkedNode| -> Option<CursorContext> {
            let mut current = Some(start_leaf);
            while let Some(node) = current {
                match node.kind() {
                    SyntaxKind::Math => return Some(CursorContext::InMath),
                    // Nested content block or top-level markup stops walk
                    SyntaxKind::ContentBlock | SyntaxKind::Markup => {
                        return None;
                    }
                    SyntaxKind::Args => {
                        let left_paren = node.children().find(|c| {
                            c.kind() == SyntaxKind::LeftParen
                                || (c.kind() == SyntaxKind::Error && &content[c.range()] == "(")
                        });
                        let right_paren =
                            node.children().find(|c| c.kind() == SyntaxKind::RightParen);

                        if let Some(lp) = left_paren {
                            let after_open = clamped >= lp.range().end;
                            let before_close = right_paren
                                .map(|rp| clamped <= rp.range().start)
                                .unwrap_or(true);

                            if after_open && before_close {
                                if let Some(parent) = node.parent() {
                                    if matches!(
                                        parent.kind(),
                                        SyntaxKind::FuncCall | SyntaxKind::SetRule
                                    ) {
                                        let name = parent
                                            .children()
                                            .find(|c| {
                                                !matches!(
                                                    c.kind(),
                                                    SyntaxKind::Set
                                                        | SyntaxKind::Space
                                                        | SyntaxKind::Args
                                                )
                                            })
                                            .map(|c| content[c.range()].trim().to_string())
                                            .unwrap_or_default();

                                        let supplied_named_args = node
                                            .children()
                                            .filter(|c| c.kind() == SyntaxKind::Named)
                                            .filter_map(|c| {
                                                c.cast::<ast::Named>()
                                                    .map(|n| n.name().as_str().to_string())
                                            })
                                            .collect();

                                        let active_param_index = node
                                            .children()
                                            .filter(|c| {
                                                c.kind() == SyntaxKind::Comma
                                                    && c.range().end <= clamped
                                            })
                                            .count();

                                        return Some(CursorContext::InFunctionCall {
                                            name,
                                            supplied_named_args,
                                            active_param_index,
                                        });
                                    }
                                }
                            }
                        }
                    }
                    SyntaxKind::Code | SyntaxKind::CodeBlock => {
                        return Some(CursorContext::InCode);
                    }
                    _ => {}
                }
                current = node.parent().cloned();
            }
            None
        };

        // Try direct leaf first
        if let Some(ctx) = classify(leaf.clone()) {
            return ctx;
        }

        // If cursor is on a trailing space of an unclosed call, try the preceding leaf
        if leaf.kind() == SyntaxKind::Space {
            if let Some(prev) = leaf.prev_leaf() {
                if let Some(ctx) = classify(prev) {
                    return ctx;
                }
            }
        }

        CursorContext::InMarkup
    }

    /// Retrieves hover tooltip/documentation at `offset`.
    #[cfg(feature = "intel")]
    pub fn tooltip(
        world: &dyn IdeWorld,
        document: Option<&PagedDocument>,
        source: &Source,
        offset: usize,
    ) -> Option<Tooltip> {
        typst_ide::tooltip(world, document, source, offset, Side::After)
            .or_else(|| typst_ide::tooltip(world, document, source, offset, Side::Before))
    }

    /// Maps a canvas click on `page` (0-indexed) at `(x_pt, y_pt)` back to a source/URL/position jump.
    #[cfg(feature = "intel")]
    pub fn jump_from_click(
        world: &dyn IdeWorld,
        document: &PagedDocument,
        page: usize,
        x_pt: f64,
        y_pt: f64,
    ) -> Option<TypastryJump> {
        // Bounds check
        if page >= document.pages().len() {
            return None;
        }

        let page_nr = std::num::NonZeroUsize::new(page + 1)?;
        let point = Point::new(Abs::pt(x_pt), Abs::pt(y_pt));
        let position = PagedPosition {
            page: page_nr,
            point,
        };

        let jump = typst_ide::jump_from_click(world, document, &position)?;

        Some(match jump {
            Jump::File(id, offset) => TypastryJump::File {
                vpath: id.vpath().get_with_slash().to_string(),
                offset: Some(offset),
            },
            Jump::Url(url) => TypastryJump::Url(url.to_string()),
            Jump::Position(pos) => TypastryJump::Position {
                page: pos.page.get().saturating_sub(1),
                x_pt: pos.point.x.to_pt(),
                y_pt: pos.point.y.to_pt(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inspect_cursor_contexts() {
        // Markup
        let s1 = Source::detached("Hello world");
        assert_eq!(
            TypastryInspector::inspect_cursor(&s1, 5),
            CursorContext::InMarkup
        );

        // Math
        let s2 = Source::detached("$ x + y $");
        assert_eq!(
            TypastryInspector::inspect_cursor(&s2, 4),
            CursorContext::InMath
        );

        // Code block
        let s3 = Source::detached("#{ let x = 1 }");
        assert_eq!(
            TypastryInspector::inspect_cursor(&s3, 8),
            CursorContext::InCode
        );

        // Import path
        let s4 = Source::detached("#import \"@preview/cetz:");
        assert_eq!(
            TypastryInspector::inspect_cursor(&s4, s4.text().len()),
            CursorContext::InImportPath("@preview/cetz:".to_string())
        );

        // Function call with named args & comma index
        let s5 = Source::detached("#rect(width: 10pt, fill: red, ");
        assert_eq!(
            TypastryInspector::inspect_cursor(&s5, s5.text().len()),
            CursorContext::InFunctionCall {
                name: "rect".to_string(),
                supplied_named_args: vec!["width".to_string(), "fill".to_string()],
                active_param_index: 2,
            }
        );

        // Nested content block inside function call returns InMarkup
        let s6 = Source::detached("#rect(width: 10pt)[hello ");
        assert_eq!(
            TypastryInspector::inspect_cursor(&s6, s6.text().len()),
            CursorContext::InMarkup
        );
    }
}
