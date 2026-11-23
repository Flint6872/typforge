use std::ops::Range;
use typst_syntax::{LinkedNode, Source, SyntaxKind, ast};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OffsetMapping {
    pub prose_range: Range<usize>,
    pub source_range: Range<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct ProseBlock {
    pub text: String,
    pub mappings: Vec<OffsetMapping>,
}

impl ProseBlock {
    /// Maps a byte range inside `self.text` back to the raw `Source` byte range.
    pub fn to_source_range(&self, prose_range: Range<usize>) -> Option<Range<usize>> {
        if prose_range.start >= prose_range.end {
            return None;
        }

        let mut start: Option<usize> = None;
        let mut end: Option<usize> = None;

        for m in &self.mappings {
            if prose_range.start < m.prose_range.end && prose_range.end > m.prose_range.start {
                let rel_start = prose_range.start.saturating_sub(m.prose_range.start);
                let rel_end = prose_range.end.min(m.prose_range.end) - m.prose_range.start;

                let mapped_start = m.source_range.start + rel_start;
                let mapped_end = m.source_range.start + rel_end;

                start = Some(start.map_or(mapped_start, |s| s.min(mapped_start)));
                end = Some(end.map_or(mapped_end, |e| e.max(mapped_end)));
            }
        }

        match (start, end) {
            (Some(s), Some(e)) if s < e => Some(s..e),
            _ => None,
        }
    }
}

/// Detects the document language from `#set text(lang: "...")` (defaults to `"en"`).
pub fn detect_document_language(source: &Source) -> String {
    let root = LinkedNode::new(source.root());
    let mut stack = vec![root];
    let mut detected = None;

    while let Some(node) = stack.pop() {
        if node.kind() == SyntaxKind::SetRule {
            if let Some(set_rule) = node.cast::<ast::SetRule>() {
                if let ast::Expr::Ident(ident) = set_rule.target() {
                    if ident.as_str() == "text" {
                        for arg in set_rule.args().items() {
                            if let ast::Arg::Named(named) = arg {
                                if named.name().as_str() == "lang" {
                                    if let ast::Expr::Str(s) = named.expr() {
                                        detected = Some(s.get().to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        for child in node.children().rev() {
            stack.push(child);
        }
    }

    detected.unwrap_or_else(|| "en".to_string())
}

/// Extracts checkable prose blocks from `source`, skipping code, math, labels,
/// comments, raw blocks, and function arguments while preserving exact byte offsets.
pub fn extract_prose_blocks(source: &Source) -> Vec<ProseBlock> {
    let mut blocks = Vec::new();
    let mut current = ProseBlock::default();

    let root = LinkedNode::new(source.root());
    visit_node(&root, source.text(), &mut current, &mut blocks);
    flush_block(&mut current, &mut blocks);

    blocks
}

fn flush_block(current: &mut ProseBlock, blocks: &mut Vec<ProseBlock>) {
    if !current.text.trim().is_empty() {
        blocks.push(std::mem::take(current));
    } else {
        current.text.clear();
        current.mappings.clear();
    }
}

fn push_mapped_slice(current: &mut ProseBlock, source_text: &str, range: Range<usize>) {
    let slice = &source_text[range.clone()];
    let prose_start = current.text.len();
    current.text.push_str(slice);
    let prose_end = current.text.len();

    current.mappings.push(OffsetMapping {
        prose_range: prose_start..prose_end,
        source_range: range,
    });
}

fn push_separator_space(current: &mut ProseBlock) {
    if !current.text.is_empty() && !current.text.ends_with(char::is_whitespace) {
        current.text.push(' ');
    }
}

fn visit_node(
    node: &LinkedNode<'_>,
    source_text: &str,
    current: &mut ProseBlock,
    blocks: &mut Vec<ProseBlock>,
) {
    match node.kind() {
        // Non-prose AST subtrees: skip completely, inserting a space separator
        // so surrounding words don't get glued together.
        SyntaxKind::Math
        | SyntaxKind::CodeBlock
        | SyntaxKind::ModuleImport
        | SyntaxKind::ModuleInclude
        | SyntaxKind::SetRule
        | SyntaxKind::ShowRule
        | SyntaxKind::LetBinding
        | SyntaxKind::Label
        | SyntaxKind::Ref
        | SyntaxKind::Raw
        | SyntaxKind::LineComment
        | SyntaxKind::BlockComment
        | SyntaxKind::Link => {
            push_separator_space(current);
        }

        // For `#func(arg: val)[prose]`, skip `arg: val` but recurse into `ContentBlock`s
        // (which may live directly under `FuncCall` or inside `Args`).
        SyntaxKind::FuncCall => {
            for child in node.children() {
                if child.kind() == SyntaxKind::ContentBlock {
                    visit_node(&child, source_text, current, blocks);
                } else if child.kind() == SyntaxKind::Args {
                    for arg_child in child.children() {
                        if arg_child.kind() == SyntaxKind::ContentBlock {
                            visit_node(&arg_child, source_text, current, blocks);
                        }
                    }
                }
            }
        }

        // Paragraph breaks and headings form separate prose blocks
        SyntaxKind::Parbreak => {
            flush_block(current, blocks);
        }
        SyntaxKind::Heading => {
            flush_block(current, blocks);
            for child in node.children() {
                visit_node(&child, source_text, current, blocks);
            }
            flush_block(current, blocks);
        }

        // Leaf prose tokens mapped 1:1 to source bytes
        SyntaxKind::Space => {
            if !current.text.is_empty() && !current.text.ends_with(char::is_whitespace) {
                push_mapped_slice(current, source_text, node.range());
            }
        }
        SyntaxKind::Text | SyntaxKind::SmartQuote => {
            push_mapped_slice(current, source_text, node.range());
        }

        // Linebreaks (`\`) act as whitespace separators
        SyntaxKind::Linebreak => {
            push_separator_space(current);
        }

        // Recurse into Markup, Strong, Emph, ContentBlock, ListItem, EnumItem, etc.
        _ => {
            for child in node.children() {
                visit_node(&child, source_text, current, blocks);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_document_language() {
        let s1 = Source::detached("Hello world");
        assert_eq!(detect_document_language(&s1), "en");

        let s2 = Source::detached("#set text(font: \"Inter\", lang: \"de\")\nHallo Welt");
        assert_eq!(detect_document_language(&s2), "de");
    }

    #[test]
    fn test_extract_prose_skips_code_math_labels_and_maps_offsets() {
        let src = Source::detached(
            "#set text(lang: \"en\")\n= My Hednig\n\nHello *wrold*, don't skip $x + y$ or <my-label>.",
        );
        let blocks = extract_prose_blocks(&src);

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].text, "My Hednig");
        assert_eq!(blocks[1].text, "Hello wrold, don't skip or .");

        // Verify byte range mapping for "Hednig"
        let hednig_prose_start = blocks[0].text.find("Hednig").unwrap();
        let hednig_src_range = blocks[0]
            .to_source_range(hednig_prose_start..hednig_prose_start + 6)
            .unwrap();
        assert_eq!(&src.text()[hednig_src_range], "Hednig");

        // Verify byte range mapping for "wrold" inside *wrold*
        let wrold_prose_start = blocks[1].text.find("wrold").unwrap();
        let wrold_src_range = blocks[1]
            .to_source_range(wrold_prose_start..wrold_prose_start + 5)
            .unwrap();
        assert_eq!(&src.text()[wrold_src_range], "wrold");

        // Verify contraction "don't" stayed intact
        let dont_prose_start = blocks[1].text.find("don't").unwrap();
        let dont_src_range = blocks[1]
            .to_source_range(dont_prose_start..dont_prose_start + 5)
            .unwrap();
        assert_eq!(&src.text()[dont_src_range], "don't");
    }

    #[test]
    fn test_extract_prose_from_text_func_content_block() {
        let src = Source::detached("Start #text(fill: red)[middle wrd] end.");
        let blocks = extract_prose_blocks(&src);

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].text, "Start middle wrd end.");

        let wrd_start = blocks[0].text.find("wrd").unwrap();
        let wrd_src = blocks[0].to_source_range(wrd_start..wrd_start + 3).unwrap();
        assert_eq!(&src.text()[wrd_src], "wrd");
    }
}
