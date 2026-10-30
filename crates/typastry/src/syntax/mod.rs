pub mod config;
pub mod parser;

pub use config::{
    AUTO_CLOSING_PAIRS, BRACKET_PAIRS, HIGHLIGHTS_QUERY, INJECTIONS_QUERY, LOCALS_QUERY,
};
pub use parser::{create_typst_parser, parse_typst, typst_language};
