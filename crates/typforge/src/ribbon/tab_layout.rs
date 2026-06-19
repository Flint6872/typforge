use crate::{actions::RibbonAction, ribbon::panel::RibbonPanel};
use gpui::*;
use gpui_component::ActiveTheme;

impl RibbonPanel {
    pub(super) fn render_layout_tab(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_flipped_val = self.is_flipped;
        let columns_val = self.columns;
        let colors = cx.theme().colors;
        div()
            .flex()
            .items_center()
            .gap_3()
            // Orientation Toggle
            .child(self.render_icon_button(
                if is_flipped_val {
                    "Landscape"
                } else {
                    "Portrait"
                },
                false,
                colors, // Pass colors
                cx.listener(move |_, _: &MouseDownEvent, _, cx| {
                    cx.emit(RibbonAction::SetFlipped(!is_flipped_val));
                }),
            ))
            // Columns
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(colors.muted_foreground) // Use muted theme color
                            .child("Columns:"),
                    )
                    .child(self.render_icon_button(
                        "1 Column",
                        columns_val == 1,
                        colors, // Pass colors
                        cx.listener(|_, _: &MouseDownEvent, _, cx| {
                            cx.emit(RibbonAction::SetColumns(1));
                        }),
                    ))
                    .child(self.render_icon_button(
                        "2 Columns",
                        columns_val == 2,
                        colors, // Pass colors
                        cx.listener(|_, _: &MouseDownEvent, _, cx| {
                            cx.emit(RibbonAction::SetColumns(2));
                        }),
                    )),
            )
    }
}
