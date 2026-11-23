use std::collections::HashSet;
use std::ops::Range;
use std::sync::Arc;

use harper_core::Document;
use harper_core::linting::{LintGroup, Linter};
use harper_core::spell::FstDictionary;
use spellbook::Dictionary;
use typst_syntax::Source;

use super::extractor::{ProseBlock, detect_document_language, extract_prose_blocks};
use super::{DiagnosticSeverity, DiagnosticSource, TextDiagnostic};

pub fn check_source(
    source: &Source,
    spellbook_dict: Option<&Dictionary>,
    ignored_words: &HashSet<String>,
) -> Vec<TextDiagnostic> {
    let lang = detect_document_language(source);
    let is_english = lang.starts_with("en");
    let blocks = extract_prose_blocks(source);
    let mut diagnostics = Vec::new();

    let harper_dict: Option<Arc<FstDictionary>> = if is_english {
        Some(FstDictionary::curated())
    } else {
        None
    };

    let mut harper_linter = harper_dict
        .as_ref()
        .map(|dict| LintGroup::new_curated(dict.clone(), harper_core::Dialect::American));

    for block in &blocks {
        if let Some(dict) = spellbook_dict {
            check_spellbook_block(block, dict, ignored_words, source.text(), &mut diagnostics);
        }

        if let (Some(dict), Some(linter)) = (&harper_dict, &mut harper_linter) {
            check_harper_block(block, dict, linter, source.text(), &mut diagnostics);
        }
    }

    diagnostics.sort_by_key(|d| (d.range.start, d.range.end));
    diagnostics
}

fn check_spellbook_block(
    block: &ProseBlock,
    dict: &Dictionary,
    ignored_words: &HashSet<String>,
    raw_source: &str,
    out: &mut Vec<TextDiagnostic>,
) {
    // Tokenize prose into words with byte offsets
    for (word_start, word) in tokenize_words(&block.text) {
        if ignored_words.contains(word) || ignored_words.contains(&word.to_lowercase()) {
            continue;
        }

        if !dict.check(word) {
            let prose_range = word_start..(word_start + word.len());
            let Some(source_range) = block.to_source_range(prose_range) else {
                continue;
            };

            // Fix: Create the buffer and pass it to suggest
            let mut suggestions = Vec::new();
            dict.suggest(word, &mut suggestions);

            out.push(TextDiagnostic {
                flagged_text: raw_source[source_range.clone()].to_string(),
                range: source_range,
                message: format!("Unknown word: '{word}'"),
                suggestions,
                source: DiagnosticSource::Spelling,
                severity: DiagnosticSeverity::Error,
            });
        }
    }
}

fn tokenize_words(text: &str) -> Vec<(usize, &str)> {
    let mut words = Vec::new();
    let mut start = None;

    for (i, c) in text.char_indices() {
        if c.is_alphabetic() || c == '\'' || c == '’' {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(s) = start {
            let word = &text[s..i];
            let trimmed = word.trim_matches(['\'', '’']);
            if !trimmed.is_empty() {
                let trimmed_start = s + (word.len() - word.trim_start_matches(['\'', '’']).len());
                words.push((trimmed_start, trimmed));
            }
            start = None;
        }
    }

    if let Some(s) = start {
        let word = &text[s..];
        let trimmed = word.trim_matches(['\'', '’']);
        if !trimmed.is_empty() {
            let trimmed_start = s + (word.len() - word.trim_start_matches(['\'', '’']).len());
            words.push((trimmed_start, trimmed));
        }
    }

    words
}

fn check_harper_block(
    block: &ProseBlock,
    harper_dict: &Arc<FstDictionary>,
    linter: &mut LintGroup,
    raw_source: &str,
    out: &mut Vec<TextDiagnostic>,
) {
    let doc = Document::new_plain_english(&block.text, harper_dict);
    let lints = linter.lint(&doc);

    for lint in lints {
        let Some(prose_byte_range) =
            char_span_to_byte_range(&block.text, lint.span.start, lint.span.end)
        else {
            continue;
        };
        let Some(source_range) = block.to_source_range(prose_byte_range) else {
            continue;
        };

        let suggestions: Vec<String> = lint
            .suggestions
            .iter()
            .filter_map(|s| match s {
                harper_core::linting::Suggestion::ReplaceWith(chars) => {
                    Some(chars.iter().collect::<String>())
                }
                harper_core::linting::Suggestion::Remove => Some(String::new()),
                _ => None,
            })
            .collect();

        out.push(TextDiagnostic {
            flagged_text: raw_source[source_range.clone()].to_string(),
            range: source_range,
            message: lint.message.clone(),
            suggestions,
            source: DiagnosticSource::Grammar,
            severity: DiagnosticSeverity::Warning,
        });
    }
}

fn char_span_to_byte_range(text: &str, char_start: usize, char_end: usize) -> Option<Range<usize>> {
    if char_start >= char_end {
        return None;
    }

    let mut byte_start = None;
    let mut byte_end = None;

    for (ci, (bi, _)) in text.char_indices().enumerate() {
        if ci == char_start {
            byte_start = Some(bi);
        }
        if ci == char_end {
            byte_end = Some(bi);
            break;
        }
    }

    if byte_end.is_none() && char_end == text.chars().count() {
        byte_end = Some(text.len());
    }

    match (byte_start, byte_end) {
        (Some(s), Some(e)) if s < e => Some(s..e),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_harper_grammar_maps_back_to_typst_source() {
        let src = Source::detached("This is *the the* test.");
        let diags = check_source(&src, None, &HashSet::new());

        assert!(
            !diags.is_empty(),
            "Expected harper-core to flag repeated word 'the the'"
        );
        for d in &diags {
            assert!(d.range.end <= src.text().len());
            assert_eq!(&src.text()[d.range.clone()], d.flagged_text);
        }
    }
}
