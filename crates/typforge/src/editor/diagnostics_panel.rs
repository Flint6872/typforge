use gpui_kit::base::StyledExt;
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use parking_lot::Mutex;
use std::sync::Arc;
use typst::WorldExt;
use typst::diag::{Severity, SourceDiagnostic};

/// Pre-processed diagnostic ready for rustc-style rendering.
#[derive(Clone, Debug)]
pub struct ResolvedDiagnostic {
    pub severity: Severity,
    pub message: String,
    pub file_path: Option<String>,
    pub line: Option<usize>,   // 1-based line number
    pub column: Option<usize>, // 1-based column number
    pub line_text: Option<String>,
    pub underline_start: usize, // 0-based character column offset
    pub underline_len: usize,   // Character width of underline
    pub hints: Vec<String>,
    pub trace: Vec<String>,
}

pub struct DiagnosticsPanel<W: typst_gpui::TypstGpuiWorld + typastry::IdeWorld> {
    pub diagnostics: Vec<ResolvedDiagnostic>,
    shared_world: Arc<Mutex<W>>,
    focus_handle: FocusHandle,
}

impl<W: typst_gpui::TypstGpuiWorld + typastry::IdeWorld> DiagnosticsPanel<W> {
    pub fn new(shared_world: Arc<Mutex<W>>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            diagnostics: Vec::new(),
            shared_world,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn set_diagnostics(
        &mut self,
        raw_diagnostics: Vec<SourceDiagnostic>,
        cx: &mut Context<Self>,
    ) {
        let world = self.shared_world.lock();

        self.diagnostics = raw_diagnostics
            .into_iter()
            .map(|diag| resolve_diagnostic(&*world, diag))
            .collect();

        cx.notify();
    }
}

/// Resolves a `SourceDiagnostic` using `Typst`'s World and Source text information.
fn resolve_diagnostic(
    world: &(impl typst::World + ?Sized),
    diag: SourceDiagnostic,
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
            if let Ok(source) = world.source(file_id) {
                if let Some(range) = world.range(span) {
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

impl<W: typst_gpui::TypstGpuiWorld + typastry::IdeWorld> gpui_kit::base::dock::Panel
    for DiagnosticsPanel<W>
{
    fn panel_name(&self) -> &'static str {
        "DiagnosticsPanel"
    }
}

impl<W: typst_gpui::TypstGpuiWorld + typastry::IdeWorld> component::dock::Panel
    for DiagnosticsPanel<W>
{
    fn title(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child("Diagnostics")
    }
}

impl<W: typst_gpui::TypstGpuiWorld + typastry::IdeWorld> EventEmitter<component::dock::PanelEvent>
    for DiagnosticsPanel<W>
{
}

impl<W: typst_gpui::TypstGpuiWorld + typastry::IdeWorld> Focusable for DiagnosticsPanel<W> {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl<W: typst_gpui::TypstGpuiWorld + typastry::IdeWorld> Render for DiagnosticsPanel<W> {
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
        .id(ElementId::from(format!("diag-item-{}", idx))) // <-- Unique GPUI element ID
        .flex()
        .flex_col()
        .p_3()
        .bg(rgb(0x1e1e1e))
        .border_1()
        .border_color(rgb(0x2d2d2d))
        .rounded_md()
        .font_family("Consolas")
        .text_size(px(13.0))
        // 1. Header: error: message
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
        // 2. Location arrow:  --> path:line:col
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
        // 3. Top gutter separator:  |
        .when(diag.line_text.is_some(), |this| {
            this.child(
                div()
                    .text_color(rgb(0x6272a4))
                    .child(format!(" {} |", empty_gutter)),
            )
        })
        // 4. Source code snippet with line number & caret underline
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
        // 5. Hints (= hint: ...)
        .children(diag.hints.iter().map(|hint| {
            div()
                .flex()
                .gap_1()
                .text_color(rgb(0x50fa7b))
                .child(format!(" {} = hint: ", empty_gutter))
                .child(hint.clone())
        }))
        // 6. Stack Trace (while calling function...)
        .children(diag.trace.iter().map(|tr| {
            div()
                .flex()
                .gap_1()
                .text_color(rgb(0xffb86c))
                .child(format!(" {}   ", empty_gutter))
                .child(tr.clone())
        }))
}
