use crate::{actions::RibbonAction, ribbon::panel::RibbonPanel};
use gpui::*;
use gpui_component::ActiveTheme;

impl RibbonPanel {
    pub(super) fn render_home_tab(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors;

        div()
            .flex()
            .items_center()
            .gap_3()
            // Font Group
            .child(
                div()
                    .flex()
                    .items_center()
                    .bg(colors.secondary)
                    .rounded_md()
                    .px_2()
                    .py_1()
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(colors.foreground)
                            .child(self.selected_font.clone()),
                    ),
            )
            // Font Size
            .child(
                div()
                    .flex()
                    .items_center()
                    .bg(colors.secondary)
                    .rounded_md()
                    .px_2()
                    .py_1()
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(colors.foreground)
                            .child(format!("{} pt", self.font_size)),
                    ),
            )
            // Separator
            .child(div().w(px(1.0)).h_6().bg(colors.border))
            // Buttons
            .child(self.render_icon_button(
                "B",
                self.is_bold,
                colors,
                cx.listener(|_, _, _, cx| {
                    cx.emit(RibbonAction::ToggleBold);
                }),
            ))
            .child(self.render_icon_button(
                "I",
                self.is_italic,
                colors,
                cx.listener(|_, _, _, cx| {
                    cx.emit(RibbonAction::ToggleItalic);
                }),
            ))
    }
}
