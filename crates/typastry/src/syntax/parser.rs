use tree_sitter::{Language, Parser, Tree};

/// Returns the `tree_sitter::Language` for Typst from the grammar crate.
pub fn typst_language() -> Language {
    tree_sitter_typst::LANGUAGE.into()
}

/// Creates a newly initialized `tree_sitter::Parser` configured for Typst.
pub fn create_typst_parser() -> Parser {
    let mut parser = Parser::new();
    parser
        .set_language(&typst_language())
        .expect("Error loading TreeSitterTypst grammar");
    parser
}

/// Parses a Typst source code string into a Tree-Sitter `Tree`.
pub fn parse_typst(source: &str) -> Option<Tree> {
    let mut parser = create_typst_parser();
    parser.parse(source, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typst_language_loads() {
        let mut parser = Parser::new();
        assert!(parser.set_language(&typst_language()).is_ok());
    }

    #[test]
    fn test_parse_typst_basic() {
        let source = "= Heading\nHello *world*!";
        let tree = parse_typst(source).expect("Failed to parse Typst source");
        let root = tree.root_node();
        assert_eq!(root.kind(), "source");
        assert!(!root.has_error());
    }
}
