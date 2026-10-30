Multi-Phase Implementation Plan: tree-sitter-typst & gpui_kit Integration
Architecture Overview
[crates/typastry]
   └── src/syntax/
        ├── mod.rs         --> Main module & public API re-exports
        ├── parser.rs      --> Tree-sitter parser initialization & helpers
        ├── config.rs      --> Auto-closing pairs, brackets, indentation rules
        └── ast_bridge.rs  --> Bridge between typst_syntax AST & tree-sitter nodes

[crates/typforge]
   └── src/main.rs / init  --> Registers Typst language config with gpui_kit
Phase 1: Basic Tree-Sitter Integration in typastry
Goal
Establish tree-sitter-typst parsing capabilities inside typastry and verify parsing works via unit tests.
Implementation Steps
Create crates/typastry/src/syntax/mod.rs and crates/typastry/src/syntax/parser.rs:
crates/typastry/src/syntax/parser.rs:
use tree_sitter::{Language, Parser, Tree};

/// Returns the `tree_sitter::Language` for Typst.
pub fn typst_language() -> Language {
    tree_sitter_typst::LANGUAGE.into()
}

/// Creates a newly initialized `tree_sitter::Parser` configured for Typst.
pub fn create_typst_parser() -> Parser {
    let mut parser = Parser::new();
    parser
        .set_language(&typst_language())
        .expect("Error loading TreeSitterTypst parser");
    parser
}

/// Parses a Typst source code string into a Tree-Sitter `Tree`.
pub fn parse_typst(source: &str) -> Option<Tree> {
    let mut parser = create_typst_parser();
    parser.parse(source, None)
}
Expose module in crates/typastry/src/syntax/mod.rs:
pub mod parser;

pub use parser::{create_typst_parser, parse_typst, typst_language};
Register module in crates/typastry/src/lib.rs:
pub mod syntax;
Verification Criteria (Phase 1)
Run unit test inside crates/typastry/src/syntax/parser.rs:
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_typst_basic() {
        let source = "= Heading\nHello *world*!";
        let tree = parse_typst(source).expect("Failed to parse Typst source");
        let root = tree.root_node();
        assert_eq!(root.kind(), "source");
        assert!(!root.has_error());
    }
}
Command: cargo test -p typastry syntax::parser::tests::test_parse_typst_basic
Pass Condition: Zero compilation errors; test passes.
Phase 2: Typst Language Configuration & Highlights Queries
Goal
Define auto-closing pairs, bracket pairs, indentation rules, and syntax highlight query strings in typastry for consumption by gpui_kit.
Implementation Steps
Create crates/typastry/src/syntax/config.rs:
use gpui_kit::component::input::{
    AutoClosingPair, BracketPair, IndentationRules, LanguageConfig, SyntaxContext,
};
use regex::Regex;
use std::sync::LazyLock;

static TYPST_INDENT: LazyLock<IndentationRules> = LazyLock::new(|| {
    IndentationRules::new(
        Regex::new(r"[\{\(\[]\s*$").unwrap(),
        Regex::new(r"^\s*[\}\)\]]").unwrap(),
    )
});

/// Generates the `LanguageConfig` for Typst editing rules in GPUI editor controls.
pub fn typst_language_config() -> LanguageConfig {
    LanguageConfig::default()
        .brackets([
            BracketPair::new("(", ")"),
            BracketPair::new("[", "]"),
            BracketPair::new("{", "}"),
        ])
        .auto_closing_pairs(
            [
                AutoClosingPair::new("(", ")"),
                AutoClosingPair::new("[", "]"),
                AutoClosingPair::new("{", "}"),
                AutoClosingPair::new("\"", "\""),
                AutoClosingPair::new("$", "$"),
                AutoClosingPair::new("/*", "*/")
                    .not_in([SyntaxContext::String, SyntaxContext::Comment]),
            ]
        )
        .indentation_rules(TYPST_INDENT.clone())
}

/// Embedded highlight query string from tree-sitter-typst.
pub const HIGHLIGHTS_QUERY: &str = tree_sitter_typst::HIGHLIGHTS_QUERY;
Re-export in crates/typastry/src/syntax/mod.rs:
pub mod config;
pub mod parser;

pub use config::{typst_language_config, HIGHLIGHTS_QUERY};
pub use parser::{create_typst_parser, parse_typst, typst_language};
Verification Criteria (Phase 2)
Add unit tests in config.rs:
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typst_language_config() {
        let cfg = typst_language_config();
        assert!(!cfg.brackets.is_empty());
        assert!(!HIGHLIGHTS_QUERY.is_empty());
    }
}
Command: cargo test -p typastry syntax::config::tests::test_typst_language_config
Pass Condition: Zero compilation errors; test passes.
Phase 3: Registration with gpui_kit Application Runtime
Goal
Register the typst editing configuration with gpui_kit on application startup so that EditorState::new(window, cx).language("typst") uses the new rules.
Implementation Steps
In crates/typforge/src/main.rs (or app init function):
use gpui_kit::component::input::set_language_config;
use typastry::syntax::typst_language_config;

pub fn init_languages(cx: &mut gpui::App) {
    set_language_config("typst", typastry::syntax::typst_language_config(), cx);
}
Call init_languages inside application setup:
cx.update(|cx| {
    init_languages(cx);
});
Verification Criteria (Phase 3)
Command: cargo check -p typforge
Pass Condition: Workspace compiles cleanly.
Runtime Verification: Opening or creating a .typ file sets EditorState with "typst" language and automatically applies bracket matching and auto-closing $, [, {, (.
Phase 4: Native Typst AST Bridge (typst_syntax <-> tree-sitter)
Goal
Bridge native typst_syntax::SyntaxNode (used by Typst compiler and typastry mutators) with tree-sitter::Node (used by editor engine) by alignment of byte ranges.
Implementation Steps
Create crates/typastry/src/syntax/ast_bridge.rs:
use std::ops::Range;
use tree_sitter::Node;

/// Find the smallest tree-sitter node that fully encloses a given UTF-8 byte range.
pub fn find_enclosing_node<'a>(root: Node<'a>, range: Range<usize>) -> Option<Node<'a>> {
    root.descendant_for_byte_range(range.start, range.end)
}

/// Maps a `typst_syntax::Span` / byte range to corresponding Tree-Sitter node kinds.
pub struct AstNodeMapping<'a> {
    pub ts_node: Node<'a>,
    pub byte_range: Range<usize>,
}

impl<'a> AstNodeMapping<'a> {
    pub fn from_range(tree: &'a tree_sitter::Tree, range: Range<usize>) -> Option<Self> {
        let ts_node = find_enclosing_node(tree.root_node(), range.clone())?;
        Some(Self {
            ts_node,
            byte_range: range,
        })
    }
}
Re-export in crates/typastry/src/syntax/mod.rs:
pub mod ast_bridge;
pub mod config;
pub mod parser;

pub use ast_bridge::{find_enclosing_node, AstNodeMapping};
pub use config::{typst_language_config, HIGHLIGHTS_QUERY};
pub use parser::{create_typst_parser, parse_typst, typst_language};
Verification Criteria (Phase 4)
Unit test in ast_bridge.rs:
#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::parse_typst;

    #[test]
    fn test_ast_node_mapping() {
        let code = "= Header\nSome #bold[text]";
        let tree = parse_typst(code).unwrap();
        let mapping = AstNodeMapping::from_range(&tree, 0..8);
        assert!(mapping.is_some());
    }
}
Command: cargo test -p typastry syntax::ast_bridge::tests::test_ast_node_mapping
Pass Condition: Zero compilation errors; all tests in typastry pass (cargo test -p typastry).
Handoff Summary Prompt
When handing off to another conversation or agent, copy the following block:
Please implement Phase 1 through Phase 4 of the tree-sitter-typst integration plan for typastry:
1. Create `crates/typastry/src/syntax/parser.rs` using `tree_sitter_typst::LANGUAGE`.
2. Create `crates/typastry/src/syntax/config.rs` with `typst_language_config()` and `HIGHLIGHTS_QUERY`.
3. Register `set_language_config("typst", typastry::syntax::typst_language_config(), cx)` during typforge startup.
4. Implement `crates/typastry/src/syntax/ast_bridge.rs` for AST range mapping.
Confirm that `cargo test -p typastry` and `cargo check -p typforge` pass after each phase.
