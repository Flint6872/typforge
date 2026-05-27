// crates/typforge/src/ribbon/panel.rs

use crate::actions::RibbonAction;
use gpui::*;
use gpui_component::{
    ActiveTheme, ThemeColor,
    color_picker::{ColorPickerEvent, ColorPickerState},
};

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
    #[allow(dead_code)] //will build this out at a later time
    pub paper_size: String,
    pub is_flipped: bool,
    pub columns: usize,
    pub(super) text_color_picker: Entity<ColorPickerState>,
}

impl RibbonPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let text_color_picker = cx.new(|cx| {
            ColorPickerState::new(window, cx).default_value(cx.theme().colors.foreground) // Match active theme text color
        });

        // Subscribe to color selection events
        cx.subscribe(&text_color_picker, |_, _, event, cx| {
            if let ColorPickerEvent::Change(Some(color)) = event {
                // Convert GPUI Hsla to a Typst-friendly hex RGB string (e.g. rgb("#FF6B6B"))
                let rgba = color.to_rgb();
                let hex_color = format!(
                    "rgb(\"#{:02x}{:02x}{:02x}\")",
                    (rgba.r * 255.0).round() as u8,
                    (rgba.g * 255.0).round() as u8,
                    (rgba.b * 255.0).round() as u8
                );

                cx.emit(RibbonAction::SetTextColor(hex_color));
            }
        })
        .detach();

        Self {
            active_tab: RibbonTab::Home,
            selected_font: "Liberation Sans".to_string(),
            font_size: 11.0,
            is_bold: false,
            is_italic: false,
            paper_size: "us-letter".to_string(),
            is_flipped: false,
            columns: 1,
            text_color_picker,
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
