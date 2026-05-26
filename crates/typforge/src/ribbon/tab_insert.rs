use crate::{actions::RibbonAction, ribbon::panel::RibbonPanel};
use gpui::*;
use gpui_component::ActiveTheme;

impl RibbonPanel {
    pub(super) fn render_insert_tab(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors;

        div()
            .flex()
            .items_center()
            .gap_3()
            // Grid Preset Button
            .child(self.render_icon_button(
                "Insert 3x3 Grid",
                false,
                colors,
                cx.listener(|_, _: &MouseDownEvent, _, cx| {
                    cx.emit(RibbonAction::InsertGrid { rows: 3, cols: 3 });
                }),
            ))
    }
}
