use gpui_kit::SharedString;
use gpui_kit::component::highlighter::{GrammarConfig, LanguageRegistry};
use gpui_kit::component::input::{
    AutoClosingPair, BracketPair, SyntaxContext, language_config::LanguageConfig,
    set_language_config,
};
use typastry::syntax::{
    AUTO_CLOSING_PAIRS, BRACKET_PAIRS, HIGHLIGHTS_QUERY, INJECTIONS_QUERY, LOCALS_QUERY,
    typst_language,
};

/// Creates the `LanguageConfig` for Typst editing rules (brackets, auto-closing pairs).
pub fn typst_language_config() -> LanguageConfig {
    let brackets = BRACKET_PAIRS
        .iter()
        .map(|&(open, close)| BracketPair::new(open, close));

    let auto_closing = AUTO_CLOSING_PAIRS.iter().map(|&(open, close)| {
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

/// Initializes Typst editing rules and tree-sitter syntax highlighting.
pub fn init_editor_languages(cx: &mut gpui_kit::App) {
    // 1. Register editing mechanics (brackets, auto-closing pairs)
    set_language_config("typst", typst_language_config(), cx);

    // 2. Register Tree-Sitter grammar with LanguageRegistry
    let grammar = GrammarConfig {
        name: "typst".into(),
        language: Some(typst_language()),
        injection_languages: Vec::<SharedString>::new(),
        highlights: HIGHLIGHTS_QUERY.into(),
        injections: INJECTIONS_QUERY.into(),
        locals: LOCALS_QUERY.into(),
    };

    LanguageRegistry::singleton().register("typst", &grammar);
}
