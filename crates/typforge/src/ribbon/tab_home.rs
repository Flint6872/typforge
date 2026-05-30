use crate::{
    actions::{self, RibbonAction},
    ribbon::panel::RibbonPanel,
};
use gpui::*;
use gpui_component::{ActiveTheme, IconName, Sizable, color_picker::ColorPicker, h_flex, v_flex};

impl RibbonPanel {
    pub fn handle_toggle_bold(&mut self, _: &actions::ToggleBold, cx: &mut Context<Self>) {
        cx.emit(RibbonAction::ToggleBold);
    }

    pub fn handle_toggle_italic(&mut self, _: &actions::ToggleItalic, cx: &mut Context<Self>) {
        cx.emit(RibbonAction::ToggleItalic);
    }

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
            // Separator
            .child(div().w(px(1.0)).h_6().bg(colors.border))
            // Color Picker Dropdown Component
            .child(self.render_custom_color_picker(cx))
    }
}

impl RibbonPanel {
    fn render_custom_color_picker(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors;
        let current_color = self
            .text_color_picker
            .read(cx)
            .value()
            .unwrap_or(colors.foreground);

        h_flex()
            .items_center()
            .gap_1()
            .px_2()
            .py_1()
            .rounded_md()
            .hover(|style| style.bg(colors.primary_hover))
            // This is the "Font Color" button (Icon + Underline)
            .child(
                v_flex()
                    .items_center()
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_this, _, _, _| {
                            // Optional: re-apply color logic
                        }),
                    )
                    .child(
                        div()
                            .text_size(px(16.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(colors.foreground)
                            .child("A"),
                    )
                    .child(div().w(px(18.0)).h(px(5.0)).bg(current_color)),
            )
            // The Dropdown Trigger
            .child(
                ColorPicker::new(&self.text_color_picker)
                    .small()
                    .icon(IconName::ChevronDown)
                    .anchor(Anchor::BottomLeft),
            )
    }
}
