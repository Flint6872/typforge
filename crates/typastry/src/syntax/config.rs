/// Standard delimiter pairs for Typst code blocks and groupings.
pub const BRACKET_PAIRS: &[(&str, &str)] = &[("(", ")"), ("[", "]"), ("{", "}")];

/// Pairs that should auto-close upon typing the opening delimiter.
pub const AUTO_CLOSING_PAIRS: &[(&str, &str)] = &[
    ("(", ")"),
    ("[", "]"),
    ("{", "}"),
    ("\"", "\""),
    ("$", "$"),
    ("/*", "*/"),
];

/// Embedded highlight query string from tree-sitter-typst.
pub const HIGHLIGHTS_QUERY: &str = tree_sitter_typst::HIGHLIGHTS_QUERY;
pub const INJECTIONS_QUERY: &str = tree_sitter_typst::INJECTIONS_QUERY;
pub const LOCALS_QUERY: &str = tree_sitter_typst::LOCALS_QUERY;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syntax_constants() {
        assert_eq!(BRACKET_PAIRS.len(), 3);
        assert!(!AUTO_CLOSING_PAIRS.is_empty());
        assert!(!HIGHLIGHTS_QUERY.is_empty());
    }
}
