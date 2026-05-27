use crate::{
    actions::{self, RibbonAction},
    ribbon::panel::RibbonPanel,
};
use gpui::*;
use gpui_component::{ActiveTheme, IconName, Sizable, color_picker::ColorPicker};

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
            .child(
                ColorPicker::new(&self.text_color_picker)
                    //.icon(IconName::Type) // Displays a standard text icon
                    .small()
                    .anchor(Anchor::BottomLeft),
            )
    }
}
