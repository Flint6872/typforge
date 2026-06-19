use crate::{
    actions::{self, RibbonAction},
    ribbon::panel::RibbonPanel,
};
use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable, button::Button, color_picker::ColorPicker, h_flex,
    popover::Popover, scroll::ScrollableElement, v_flex,
};

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
                Popover::new("font-family-dropdown")
                    .open(self.font_dropdown_open)
                    .on_open_change(cx.listener(|this, open: &bool, _, cx| {
                        this.font_dropdown_open = *open;
                        if !*open {
                            // Dropdown closed: if we haven't confirmed a choice, revert the preview
                            if let Some(orig) = this.original_font.take() {
                                this.selected_font = orig.clone();
                                cx.emit(RibbonAction::SetFont(orig));
                            }
                        } else {
                            // Dropdown opened: capture original family
                            this.original_font = Some(this.selected_font.clone());
                        }
                        cx.notify();
                    }))
                    .trigger(
                        Button::new("trigger-font-family").child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .bg(colors.secondary)
                                .rounded_md()
                                .px_2()
                                .py_1()
                                .cursor_pointer()
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(colors.foreground)
                                        .child(self.selected_font.clone()),
                                )
                                .child(Icon::new(IconName::ChevronDown).small()),
                        ),
                    )
                    .child(
                        v_flex()
                            .w_48()
                            .h_64()
                            .overflow_y_scrollbar()
                            .bg(colors.popover)
                            .border_1()
                            .border_color(colors.border)
                            .rounded_md()
                            .p_1()
                            .children(self.font_families.iter().map(|font| {
                                let font = font.clone();
                                let is_selected = self.selected_font == font;

                                div()
                                    .px_2()
                                    .py_1()
                                    .rounded_sm()
                                    // Use a simple transition-like color change
                                    .bg(if is_selected {
                                        colors.accent
                                    } else {
                                        transparent_black().into()
                                    })
                                    .text_color(if is_selected {
                                        colors.accent_foreground
                                    } else {
                                        colors.foreground
                                    })
                                    // Apply hover ONLY to the background, don't trigger logic here
                                    .hover(|style| style.bg(colors.primary_hover))
                                    .cursor_pointer()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener({
                                            let font = font.clone();
                                            move |this, _, _, cx| {
                                                this.selected_font = font.clone();
                                                this.font_dropdown_open = false;
                                                cx.emit(RibbonAction::SetFont(font.clone()));
                                                cx.notify();
                                            }
                                        }),
                                    )
                                    // REMOVE on_mouse_move for now to test performance.
                                    // If you MUST have preview, use a simple `cx.spawn` to debounce it by 100ms
                                    .child(div().font_family(font.clone()).child(font))
                            })),
                    ),
            )
            // Font Size
            .child(
                Popover::new("font-size-dropdown")
                    .open(self.font_size_dropdown_open)
                    .on_open_change(cx.listener(|this, open: &bool, _, cx| {
                        this.font_size_dropdown_open = *open;
                        if !*open {
                            // Dropdown closed: revert the size preview if not committed
                            if let Some(orig) = this.original_font_size.take() {
                                this.font_size = orig;
                                cx.emit(RibbonAction::SetFontSize(orig));
                            }
                        } else {
                            this.original_font_size = Some(this.font_size);
                        }
                        cx.notify();
                    }))
                    .trigger(
                        Button::new("trigger-font-size").child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .bg(colors.secondary)
                                .rounded_md()
                                .px_2()
                                .py_1()
                                .cursor_pointer()
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(colors.foreground)
                                        .child(format!("{} pt", self.font_size)),
                                )
                                .child(Icon::new(IconName::ChevronDown).small()),
                        ),
                    )
                    .child(
                        v_flex()
                            .w_20()
                            .h_48()
                            .overflow_y_scrollbar()
                            .bg(colors.popover)
                            .border_1()
                            .border_color(colors.border)
                            .rounded_md()
                            .p_1()
                            .children(
                                [
                                    8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0,
                                    36.0, 48.0, 72.0,
                                ]
                                .iter()
                                .map(|&size| {
                                    let is_selected = self.font_size == size;

                                    div()
                                        .px_2()
                                        .py_1()
                                        .rounded_sm()
                                        .bg(if is_selected {
                                            colors.accent
                                        } else {
                                            transparent_black().into()
                                        })
                                        .text_color(if is_selected {
                                            colors.accent_foreground
                                        } else {
                                            colors.foreground
                                        })
                                        .cursor_pointer()
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |this, _, _, cx| {
                                                this.font_size = size;
                                                this.original_font_size = None; // Commit changes
                                                this.font_size_dropdown_open = false;
                                                cx.emit(RibbonAction::SetFontSize(size));
                                                cx.notify();
                                            }),
                                        )
                                        .on_mouse_move(cx.listener(move |_, _, _, cx| {
                                            //  cx.emit(RibbonAction::SetFontSize(size));
                                        }))
                                        .child(format!("{}", size))
                                }),
                            ),
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

        // This outer container acts as the "unified button"
        h_flex()
            .items_center()
            .rounded_md()
            .border_1()
            .border_color(colors.border) // Optional: adds definition
            .hover(|style| style.bg(colors.primary_hover)) // Hover the whole unit
            .child(
                // Left Side: Apply Action
                v_flex()
                    .items_center()
                    .px_2()
                    .py_1()
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            if let Some(color) = this.text_color_picker.read(cx).value() {
                                this.emit_text_color(color, cx);
                            }
                        }),
                    )
                    .child(
                        div()
                            .text_size(px(16.0))
                            .font_weight(FontWeight::BOLD)
                            .child("A"),
                    )
                    .child(div().w(px(18.0)).h(px(5.0)).bg(current_color)),
            )
            .child(
                // Divider line between sides
                div().w(px(1.0)).h(px(24.0)).bg(colors.border),
            )
            .child(
                // Right Side: Dropdown
                div().px_1().child(
                    ColorPicker::new(&self.text_color_picker)
                        .small()
                        .icon(IconName::ChevronDown)
                        .anchor(Anchor::BottomLeft),
                ),
            )
            .cursor_pointer()
    }
}
