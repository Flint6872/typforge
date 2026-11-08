pub mod config;
pub mod parser;

pub use config::{
    BIBTEX_AUTO_CLOSING_PAIRS, BIBTEX_BRACKET_PAIRS, BIBTEX_HIGHLIGHTS_QUERY, BIBTEX_LOCALS_QUERY,
    TYPST_AUTO_CLOSING_PAIRS, TYPST_BRACKET_PAIRS, TYPST_HIGHLIGHTS_QUERY, TYPST_INJECTIONS_QUERY,
    TYPST_LOCALS_QUERY,
};
pub use parser::{bibtex_language, create_typst_parser, parse_typst, typst_language};
