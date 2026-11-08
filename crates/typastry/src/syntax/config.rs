/// Standard delimiter pairs for Typst code blocks and groupings.
pub const TYPST_BRACKET_PAIRS: &[(&str, &str)] = &[("(", ")"), ("[", "]"), ("{", "}")];

/// Pairs that should auto-close upon typing the opening delimiter.
pub const TYPST_AUTO_CLOSING_PAIRS: &[(&str, &str)] = &[
    ("(", ")"),
    ("[", "]"),
    ("{", "}"),
    ("\"", "\""),
    ("$", "$"),
    ("/*", "*/"),
];

/// Embedded highlight query string from tree-sitter-typst.
pub const TYPST_HIGHLIGHTS_QUERY: &str = tree_sitter_typst::HIGHLIGHTS_QUERY;
pub const TYPST_INJECTIONS_QUERY: &str = tree_sitter_typst::INJECTIONS_QUERY;
pub const TYPST_LOCALS_QUERY: &str = tree_sitter_typst::LOCALS_QUERY;

/// Standard delimiter pairs for BibTeX entries and fields.
pub const BIBTEX_BRACKET_PAIRS: &[(&str, &str)] = &[("(", ")"), ("{", "}")];

/// Pairs that should auto-close upon typing the opening delimiter in BibTeX.
pub const BIBTEX_AUTO_CLOSING_PAIRS: &[(&str, &str)] = &[("(", ")"), ("{", "}"), ("\"", "\"")];

/// Embedded highlight query string from tree-sitter-bibtex.
pub const BIBTEX_HIGHLIGHTS_QUERY: &str = include_str!("../../../../assets/bibtex/highlights.scm");
pub const BIBTEX_LOCALS_QUERY: &str = include_str!("../../../../assets/bibtex/locals.scm");
pub const BIBTEX_TAS_QUERY: &str = include_str!("../../../../assets/bibtex/tags.scm");

//pub const BIBTEX_HIGHLIGHTS_QUERY: &str = tree_sitter_bibtex::

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syntax_constants() {
        assert_eq!(TYPST_BRACKET_PAIRS.len(), 3);
        assert!(!TYPST_AUTO_CLOSING_PAIRS.is_empty());
        assert!(!TYPST_HIGHLIGHTS_QUERY.is_empty());
    }

    #[test]
    fn test_bibtex_syntax_constants() {
        assert_eq!(BIBTEX_BRACKET_PAIRS.len(), 2);
        assert!(!BIBTEX_AUTO_CLOSING_PAIRS.is_empty());
        assert!(!BIBTEX_HIGHLIGHTS_QUERY.is_empty());
        // Uncomment the following if tree-sitter-bibtex provides these
        // assert!(!BIBTEX_INJECTIONS_QUERY.is_empty());
        // assert!(!BIBTEX_LOCALS_QUERY.is_empty());
    }
}
