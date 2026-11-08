use gpui_kit::SharedString;
use gpui_kit::component::highlighter::{GrammarConfig, LanguageRegistry};
use gpui_kit::component::input::{
    AutoClosingPair, BracketPair, SyntaxContext, language_config::LanguageConfig,
    set_language_config,
};
use typastry::syntax::{
    BIBTEX_AUTO_CLOSING_PAIRS, BIBTEX_BRACKET_PAIRS, BIBTEX_HIGHLIGHTS_QUERY, BIBTEX_LOCALS_QUERY,
    TYPST_AUTO_CLOSING_PAIRS, TYPST_BRACKET_PAIRS, TYPST_HIGHLIGHTS_QUERY, TYPST_INJECTIONS_QUERY,
    TYPST_LOCALS_QUERY, bibtex_language, typst_language,
};

/// Creates the `LanguageConfig` for Typst editing rules (brackets, auto-closing pairs).
pub fn typst_language_config() -> LanguageConfig {
    let brackets = TYPST_BRACKET_PAIRS
        .iter()
        .map(|&(open, close)| BracketPair::new(open, close));

    let auto_closing = TYPST_AUTO_CLOSING_PAIRS.iter().map(|&(open, close)| {
        if open == "/*" {
            AutoClosingPair::new(open, close)
                .not_in([SyntaxContext::String, SyntaxContext::Comment])
        } else {
            AutoClosingPair::new(open, close)
        }
    });

    LanguageConfig::default()
        .brackets(brackets)
        .auto_closing_pairs(auto_closing)
}

/// Creates the `LanguageConfig` for BibTeX editing rules (brackets, auto-closing pairs).
pub fn bibtex_language_config() -> LanguageConfig {
    let brackets = BIBTEX_BRACKET_PAIRS
        .iter()
        .map(|&(open, close)| BracketPair::new(open, close));

    let auto_closing = BIBTEX_AUTO_CLOSING_PAIRS
        .iter()
        .map(|&(open, close)| AutoClosingPair::new(open, close));

    LanguageConfig::default()
        .brackets(brackets)
        .auto_closing_pairs(auto_closing)
}

/// Initializes Typst editing rules and tree-sitter syntax highlighting.
pub fn init_editor_languages(cx: &mut gpui_kit::App) {
    // 1. Register editing mechanics (brackets, auto-closing pairs)
    set_language_config("typst", typst_language_config(), cx);

    // 2. Register Tree-Sitter grammar with LanguageRegistry
    let grammar = GrammarConfig {
        name: "typst".into(),
        language: Some(typst_language()),
        injection_languages: Vec::<SharedString>::new(),
        highlights: TYPST_HIGHLIGHTS_QUERY.into(),
        injections: TYPST_INJECTIONS_QUERY.into(),
        locals: TYPST_LOCALS_QUERY.into(),
    };

    LanguageRegistry::singleton().register("typst", &grammar);

    set_language_config("bibtex", bibtex_language_config(), cx);

    //Register Tree-Sitter grammar for BibTeX
    let bibtex_grammar = GrammarConfig {
        name: "bibtex".into(),
        language: Some(bibtex_language()),
        // BibTeX generally doesn't inject other languages.
        injection_languages: Vec::<SharedString>::new(),
        highlights: BIBTEX_HIGHLIGHTS_QUERY.into(),
        injections: "".into(),
        locals: BIBTEX_LOCALS_QUERY.into(),
    };
    LanguageRegistry::singleton().register("bibtex", &bibtex_grammar);
}
