pub mod extractor;

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
    /// Byte range in the raw Typst `Source` text.
    pub range: Range<usize>,
    pub message: String,
    pub suggestions: Vec<String>,
    pub source: DiagnosticSource,
    pub severity: DiagnosticSeverity,
    /// Exact slice from the raw source (useful for "Ignore Word" / "Add to Dictionary").
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
