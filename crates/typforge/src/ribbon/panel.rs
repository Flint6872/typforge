// crates/typforge/src/ribbon/panel.rs

use crate::actions::RibbonAction;
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
    pub selected_font: String,
    pub font_size: f32,
    pub is_bold: bool,
    pub is_italic: bool,
    pub paper_size: String,
    pub is_flipped: bool,
    pub columns: usize,
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

    pub fn render_icon_button(
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
