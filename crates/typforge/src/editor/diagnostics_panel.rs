use gpui_kit::base::StyledExt;
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::collections::HashMap;
use std::sync::Arc;
use typforge_core::{ResolvedDiagnostic, resolve_diagnostics};
use typst::diag::{Severity, SourceDiagnostic};
use typst::syntax::{FileId, Source};

pub struct DiagnosticsPanel {
    pub diagnostics: Vec<ResolvedDiagnostic>,
    focus_handle: FocusHandle,
}

impl DiagnosticsPanel {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            diagnostics: Vec::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn set_diagnostics(
        &mut self,
        sources: &HashMap<FileId, Arc<Source>>,
        raw_diagnostics: Vec<SourceDiagnostic>,
        cx: &mut Context<Self>,
    ) {
        self.diagnostics = resolve_diagnostics(sources, &raw_diagnostics);
        cx.notify();
    }
}

impl gpui_kit::base::dock::Panel for DiagnosticsPanel {
    fn panel_name(&self) -> &'static str {
        "DiagnosticsPanel"
    }
}

impl component::dock::Panel for DiagnosticsPanel {
    fn title(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child("Diagnostics")
    }
}

impl EventEmitter<component::dock::PanelEvent> for DiagnosticsPanel {}

impl Focusable for DiagnosticsPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for DiagnosticsPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("diagnostics-panel-root")
            .size_full()
            .bg(rgb(0x181818))
            .track_focus(&self.focus_handle)
            .p_3()
            .overflow_y_scrollbar()
            .child(if self.diagnostics.is_empty() {
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(0x666666))
                    .child("No diagnostics")
                    .into_any_element()
            } else {
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .children(
                        self.diagnostics
                            .iter()
                            .enumerate()
                            .map(|(idx, diag)| render_rustc_diagnostic(idx, diag)),
                    )
                    .into_any_element()
            })
    }
}

/// Renders a single diagnostic with a rustc compiler look and feel.
fn render_rustc_diagnostic(idx: usize, diag: &ResolvedDiagnostic) -> impl IntoElement {
    let (header_color, severity_name) = match diag.severity {
        Severity::Error => (rgb(0xff5555), "error"),
        Severity::Warning => (rgb(0xf1fa8c), "warning"),
    };

    let line_num_str = diag.line.map(|l| l.to_string()).unwrap_or_default();
    let gutter_width = line_num_str.len().max(2);
    let empty_gutter = " ".repeat(gutter_width);

    div()
        .id(ElementId::from(format!("diag-item-{}", idx)))
        .flex()
        .flex_col()
        .p_3()
        .bg(rgb(0x1e1e1e))
        .border_1()
        .border_color(rgb(0x2d2d2d))
        .rounded_md()
        .font_family("Consolas")
        .text_size(px(13.0))
        .child(
            div()
                .flex()
                .gap_2()
                .child(
                    div()
                        .font_semibold()
                        .text_color(header_color)
                        .child(format!("{}:", severity_name)),
                )
                .child(
                    div()
                        .font_semibold()
                        .text_color(rgb(0xf8f8f2))
                        .child(diag.message.clone()),
                ),
        )
        .when(diag.file_path.is_some(), |this| {
            let path = diag.file_path.as_deref().unwrap_or("");
            let loc_str = match (diag.line, diag.column) {
                (Some(l), Some(c)) => format!("{}:{}:{}", path, l, c),
                (Some(l), None) => format!("{}:{}", path, l),
                _ => path.to_string(),
            };

            this.child(
                div()
                    .flex()
                    .gap_1()
                    .text_color(rgb(0x8be9fd))
                    .child(format!(" {}--> ", empty_gutter))
                    .child(loc_str),
            )
        })
        .when(diag.line_text.is_some(), |this| {
            this.child(
                div()
                    .text_color(rgb(0x6272a4))
                    .child(format!(" {} |", empty_gutter)),
            )
        })
        .when_some(diag.line_text.as_ref(), |this, code| {
            let padded_line = format!("{:>width$}", line_num_str, width = gutter_width);
            let caret_padding = " ".repeat(diag.underline_start);
            let carets = "^".repeat(diag.underline_len);

            this.child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .child(
                                div()
                                    .text_color(rgb(0x6272a4))
                                    .child(format!(" {} | ", padded_line)),
                            )
                            .child(div().text_color(rgb(0xf8f8f2)).child(code.clone())),
                    )
                    .child(
                        div()
                            .flex()
                            .child(
                                div()
                                    .text_color(rgb(0x6272a4))
                                    .child(format!(" {} | ", empty_gutter)),
                            )
                            .child(
                                div()
                                    .text_color(header_color)
                                    .font_semibold()
                                    .child(format!("{}{}", caret_padding, carets)),
                            ),
                    ),
            )
        })
        .children(diag.hints.iter().map(|hint| {
            div()
                .flex()
                .gap_1()
                .text_color(rgb(0x50fa7b))
                .child(format!(" {} = hint: ", empty_gutter))
                .child(hint.clone())
        }))
        .children(diag.trace.iter().map(|tr| {
            div()
                .flex()
                .gap_1()
                .text_color(rgb(0xffb86c))
                .child(format!(" {}   ", empty_gutter))
                .child(tr.clone())
        }))
}
