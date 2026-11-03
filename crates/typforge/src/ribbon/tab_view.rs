// crates/typforge/src/ribbon/tab_view.rs

use crate::{
    actions::RibbonAction,
    ribbon::{
        RibbonIconName,
        panel::{RibbonButton, RibbonPanel},
    },
};
use gpui_kit::base::h_flex;
use gpui_kit::component::ActiveTheme;
use gpui_kit::*;

impl RibbonPanel {
    pub(super) fn render_view_tab(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_compact_val = self.is_compact;
        let colors = cx.theme().colors;

        h_flex().flex().items_center().gap_3().child(
            self.render_command_group(
                [
                    // Toggle Compact Mode
                    self.render_ribbon_button(
                        RibbonButton {
                            id: "toggle-compact-mode",
                            label: if is_compact_val {
                                None
                            } else {
                                Some("Compact Mode")
                            },
                            icon: None,
                            is_functional: true,
                            is_contextually_active: is_compact_val,
                            action: RibbonAction::SetColumns(1), // Substitute with actual RibbonAction when added
                        },
                        cx,
                    )
                    .into_any_element(),
                    // Unimplemented / Inactive Preview Buttons
                    self.render_ribbon_button(
                        RibbonButton {
                            id: "zoom-in",
                            label: if is_compact_val {
                                None
                            } else {
                                Some("Zoom In")
                            },
                            icon: None,
                            is_functional: false, // Grayed out, pointer events disabled
                            is_contextually_active: false,
                            action: RibbonAction::SetColumns(1),
                        },
                        cx,
                    )
                    .into_any_element(),
                    self.render_ribbon_button(
                        RibbonButton {
                            id: "zoom-out",
                            label: if is_compact_val {
                                None
                            } else {
                                Some("Zoom Out")
                            },
                            icon: None,
                            is_functional: false, // Grayed out, pointer events disabled
                            is_contextually_active: false,
                            action: RibbonAction::SetColumns(1),
                        },
                        cx,
                    )
                    .into_any_element(),
                ],
                colors,
            ),
        )
    }
}
