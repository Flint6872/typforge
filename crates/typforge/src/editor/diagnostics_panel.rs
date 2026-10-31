// crates/typforge/src/workspace/diagnostics_panel.rs
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::*;
use typst::diag::SourceDiagnostic;

pub struct DiagnosticsPanel {
    pub diagnostics: Vec<SourceDiagnostic>,
    focus_handle: FocusHandle,
}

impl DiagnosticsPanel {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            diagnostics: Vec::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn set_diagnostics(&mut self, diagnostics: Vec<SourceDiagnostic>, cx: &mut Context<Self>) {
        self.diagnostics = diagnostics;
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
            .bg(rgb(0x1a1a1a))
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
                    .gap_2()
                    .children(self.diagnostics.iter().map(|diag| {
                        div()
                            .p_2()
                            .bg(rgb(0x2d1a1a))
                            .border_1()
                            .border_color(rgb(0x5a2222))
                            .rounded_md()
                            .text_color(rgb(0xff5555))
                            .child(format!("Error: {}", diag.message))
                    }))
                    .into_any_element()
            })
    }
}
