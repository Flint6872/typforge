pub mod extractor;

#[cfg(feature = "spellcheck")]
pub mod dictionary;

#[cfg(feature = "spellcheck")]
pub mod checker;

#[cfg(feature = "spellcheck")]
pub use spellbook::Dictionary;

use crate::edit::TextEdit;
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DiagnosticSource {
    Spelling,
    Grammar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Suggestion,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TextDiagnostic {
    pub range: Range<usize>,
    pub message: String,
    pub suggestions: Vec<String>,
    pub source: DiagnosticSource,
    pub severity: DiagnosticSeverity,
    pub flagged_text: String,
}

impl TextDiagnostic {
    pub fn to_text_edit(&self, replacement: &str) -> TextEdit {
        let new_end = self.range.start + replacement.len();
        TextEdit {
            range: self.range.clone(),
            new_text: replacement.to_string(),
            new_selection: new_end..new_end,
        }
    }
}
