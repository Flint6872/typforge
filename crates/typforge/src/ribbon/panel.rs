// crates/typforge/src/ribbon/panel.rs

use crate::{actions::RibbonAction, components::theme};
use gpui::*;
use gpui_component::{ActiveTheme, ThemeColor};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RibbonTab {
    Home,
    PageLayout,
    Insert,
}

pub struct RibbonPanel {
    active_tab: RibbonTab,
    selected_font: String,
    font_size: f32,
    is_bold: bool,
    is_italic: bool,
    paper_size: String,
    is_flipped: bool,
    columns: usize,
}

impl RibbonPanel {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            active_tab: RibbonTab::Home,
            selected_font: "Liberation Sans".to_string(),
            font_size: 11.0,
            is_bold: false,
            is_italic: false,
            paper_size: "us-letter".to_string(),
            is_flipped: false,
            columns: 1,
        }
    }

    fn select_tab(&mut self, tab: RibbonTab, cx: &mut Context<Self>) {
        self.active_tab = tab;
        cx.notify();
    }

    // Help UI reflect changes from outside
    pub fn update_text_states(
        &mut self,
        is_bold: bool,
        is_italic: bool,
        size: f32,
        cx: &mut Context<Self>,
    ) {
        self.is_bold = is_bold;
        self.is_italic = is_italic;
        self.font_size = size;
        cx.notify();
    }
}

impl EventEmitter<RibbonAction> for RibbonPanel {}

impl Render for RibbonPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.active_tab;

        div()
            .w_full()
            .bg(cx.theme().colors.popover)
            .border_1()
            .border_color(cx.theme().colors.border)
            .flex()
            .flex_col()
            .child(
                // Tab Header Bar
                div()
                    .flex()
                    .px_4()
                    .pt_2()
                    .gap_2()
                    .bg(cx.theme().colors.tab_bar)
                    .child(self.render_tab_header("Home", RibbonTab::Home, cx))
                    .child(self.render_tab_header("Page Layout", RibbonTab::PageLayout, cx))
                    .child(self.render_tab_header("Insert", RibbonTab::Insert, cx)),
            )
            .child(
                // Tab Content Panel
                div()
                    .h_16()
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(match active_tab {
                        RibbonTab::Home => self.render_home_tab(cx).into_any_element(),
                        RibbonTab::PageLayout => self.render_layout_tab(cx).into_any_element(),
                        RibbonTab::Insert => self.render_insert_tab(cx).into_any_element(),
                    }),
            )
    }
}

impl RibbonPanel {
    fn render_tab_header(
        &self,
        label: &'static str,
        tab: RibbonTab,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_active = self.active_tab == tab;
        let colors = cx.theme().colors;

        div()
            .px_3()
            .py_1()
            .rounded_t_md()
            .text_size(px(12.0))
            .text_color(if is_active {
                colors.tab_active_foreground
            } else {
                colors.tab_foreground
            })
            .bg(if is_active {
                colors.tab_active
            } else {
                transparent_black().into()
            })
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.select_tab(tab, cx);
                }),
            )
            .child(label)
    }

    fn render_home_tab(&self, cx: &mut Context<Self>) -> impl IntoElement {
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

    fn render_layout_tab(&self, cx: &mut Context<Self>) -> impl IntoElement {
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

    fn render_insert_tab(&self, cx: &mut Context<Self>) -> impl IntoElement {
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

    fn render_icon_button(
        &self,
        label: &'static str,
        active: bool,
        colors: ThemeColor,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        div()
            .px_3()
            .py_1()
            .rounded_md()
            .bg(if active {
                colors.primary
            } else {
                colors.secondary
            })
            .border_1()
            .border_color(if active {
                colors.primary_active
            } else {
                colors.border
            })
            .text_size(px(12.0))
            .text_color(if active {
                colors.primary_foreground
            } else {
                colors.foreground
            })
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, on_click)
            .child(label)
    }
}
