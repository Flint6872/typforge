use std::collections::HashMap;
use std::sync::Arc;
use typst::diag::{Severity, SourceDiagnostic};
use typst::syntax::{DiagSpanKind, FileId, Source};

/// Pre-processed diagnostic ready for UI rendering.
#[derive(Clone, Debug)]
pub struct ResolvedDiagnostic {
    pub severity: Severity,
    pub message: String,
    pub file_path: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub line_text: Option<String>,
    pub underline_start: usize,
    pub underline_len: usize,
    pub hints: Vec<String>,
    pub trace: Vec<String>,
}

/// Resolves raw Typst diagnostics against pre-collected sources.
pub fn resolve_diagnostics(
    sources: &HashMap<FileId, Arc<Source>>,
    raw_diagnostics: &[SourceDiagnostic],
) -> Vec<ResolvedDiagnostic> {
    raw_diagnostics
        .iter()
        .filter(|diag| {
            diag.span
                .id()
                .is_some_and(|id| id.vpath().extension() == Some("typ"))
        })
        .map(|diag| resolve_single(sources, diag))
        .collect()
}

fn resolve_single(
    sources: &HashMap<FileId, Arc<Source>>,
    diag: &SourceDiagnostic,
) -> ResolvedDiagnostic {
    let mut file_path = None;
    let mut line = None;
    let mut column = None;
    let mut line_text = None;
    let mut underline_start = 0;
    let mut underline_len = 1;

    let span = diag.span;
    if !span.is_detached() {
        if let Some(file_id) = span.id() {
            file_path = Some(file_id.vpath().get_with_slash().to_string());
            if let Some(source) = sources.get(&file_id) {
                let range = match span.get() {
                    DiagSpanKind::Number { num, .. } => source.range(num, None),
                    DiagSpanKind::Range { range, .. } => Some(range),
                    _ => None,
                };

                if let Some(range) = range {
                    let text = source.text();
                    if range.start <= text.len() {
                        let prefix = &text[..range.start];
                        let line_idx = prefix.bytes().filter(|&b| b == b'\n').count();
                        let line_start_byte = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
                        let line_end_byte = text[range.start..]
                            .find('\n')
                            .map(|i| range.start + i)
                            .unwrap_or(text.len());

                        let raw_line = &text[line_start_byte..line_end_byte];
                        let trimmed_line = raw_line.trim_end_matches('\r');
                        let col_idx = text[line_start_byte..range.start].chars().count();

                        let underline_end_byte = range.end.min(line_end_byte);
                        let len = if underline_end_byte > range.start {
                            text[range.start..underline_end_byte].chars().count().max(1)
                        } else {
                            1
                        };

                        line = Some(line_idx + 1);
                        column = Some(col_idx + 1);
                        underline_start = col_idx;
                        underline_len = len;
                        line_text = Some(trimmed_line.to_string());
                    }
                }
            }
        }
    }

    let hints = diag.hints.iter().map(|h| h.v.to_string()).collect();
    let trace = diag.trace.iter().map(|t| format!("{}", t.v)).collect();

    ResolvedDiagnostic {
        severity: diag.severity,
        message: diag.message.to_string(),
        file_path,
        line,
        column,
        line_text,
        underline_start,
        underline_len,
        hints,
        trace,
    }
}
