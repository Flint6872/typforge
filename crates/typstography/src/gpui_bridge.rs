use gpui_component::highlighter as gpui_hl;
use gpui_component::input::Position as GpuiPosition;
use std::str::FromStr;
use tower_lsp::lsp_types as lsp;

// Bring in lsp_types 0.97 specifically for the Uri conversion and
// for the types nested within DiagnosticRelatedInformation
use lsp_types::{
    Location as LspTypesLocation097, Position as LspTypesPosition097, Range as LspTypesRange097,
    Uri as LspTypesUri097,
};

/// Custom converter from tower_lsp::lsp_types::Url (which is url::Url) to lsp_types::Uri (0.97).
/// This function explicitly converts the Url to a string and then parses it into Uri.
fn convert_url_to_uri(url: lsp::Url) -> LspTypesUri097 {
    // Convert the url::Url into its string representation
    let url_string = url.to_string();
    // Parse the string into lsp_types::Uri (0.97)
    LspTypesUri097::from_str(&url_string).expect("Failed to parse URL string into lsp_types::Uri")
}

/// A wrapper around GPUI's diagnostic type to allow `From` implementations
/// from the LSP types used in this crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuiDiagnostic(pub gpui_hl::Diagnostic);

impl From<lsp::Diagnostic> for GpuiDiagnostic {
    fn from(lsp_diag: lsp::Diagnostic) -> Self {
        let gpui_diagnostic = gpui_hl::Diagnostic {
            // Map lsp::Range to std::ops::Range<GpuiPosition>
            range: GpuiPosition::new(lsp_diag.range.start.line, lsp_diag.range.start.character)
                ..GpuiPosition::new(lsp_diag.range.end.line, lsp_diag.range.end.character),

            // Map lsp::DiagnosticSeverity to gpui_hl::DiagnosticSeverity
            severity: match lsp_diag.severity {
                Some(lsp::DiagnosticSeverity::ERROR) => gpui_hl::DiagnosticSeverity::Error,
                Some(lsp::DiagnosticSeverity::WARNING) => gpui_hl::DiagnosticSeverity::Warning,
                Some(lsp::DiagnosticSeverity::INFORMATION) => gpui_hl::DiagnosticSeverity::Info,
                Some(lsp::DiagnosticSeverity::HINT) => gpui_hl::DiagnosticSeverity::Hint,
                _ => gpui_hl::DiagnosticSeverity::Info,
            },

            message: lsp_diag.message.into(),
            source: lsp_diag.source.map(Into::into),

            // Map lsp::NumberOrString to Option<SharedString>
            code: lsp_diag.code.map(|c| match c {
                lsp::NumberOrString::Number(n) => n.to_string().into(),
                lsp::NumberOrString::String(s) => s.into(),
            }),

            code_description: lsp_diag.code_description.map(|cd| {
                gpui_hl::CodeDescription {
                    // Use our custom converter
                    href: convert_url_to_uri(cd.href.clone()),
                }
            }),

            related_information: lsp_diag.related_information.map(|infos| {
                infos
                    .into_iter()
                    .map(|info| gpui_hl::DiagnosticRelatedInformation {
                        location: LspTypesLocation097 {
                            // Use our custom converter
                            uri: convert_url_to_uri(info.location.uri),
                            range: LspTypesRange097 {
                                start: LspTypesPosition097 {
                                    line: info.location.range.start.line,
                                    character: info.location.range.start.character,
                                },
                                end: LspTypesPosition097 {
                                    line: info.location.range.end.line,
                                    character: info.location.range.end.character,
                                },
                            },
                        },
                        message: info.message,
                    })
                    .collect()
            }),

            tags: lsp_diag.tags.map(|tags| {
                tags.into_iter()
                    .map(|tag| match tag {
                        lsp::DiagnosticTag::UNNECESSARY => gpui_hl::DiagnosticTag::UNNECESSARY,
                        lsp::DiagnosticTag::DEPRECATED => gpui_hl::DiagnosticTag::DEPRECATED,
                        _ => gpui_hl::DiagnosticTag::UNNECESSARY,
                    })
                    .collect()
            }),

            data: lsp_diag.data,
        };

        Self(gpui_diagnostic)
    }
}

// Helper for bulk conversion
pub fn map_diagnostics(lsp_diags: Vec<lsp::Diagnostic>) -> Vec<gpui_hl::Diagnostic> {
    lsp_diags
        .into_iter()
        .map(|d| GpuiDiagnostic::from(d).0)
        .collect()
}
