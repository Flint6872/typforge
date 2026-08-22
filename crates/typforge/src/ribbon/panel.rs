// crates/typforge/src/ribbon/panel.rs

use crate::actions::RibbonAction;
use gpui::*;
use gpui_component::{
    ActiveTheme, ThemeColor,
    color_picker::{ColorPickerEvent, ColorPickerState},
};
use typastry::edit::ActiveProperties;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RibbonTab {
    Home,
    PageLayout,
    Insert,
}

pub struct RibbonPanel {
    active_tab: RibbonTab,
    pub selected_font: String,
    pub original_font: Option<String>,
    pub font_size: f32,
    pub original_font_size: Option<f32>,
    pub is_bold: bool,
    pub is_italic: bool,
    pub is_underline: bool,
    #[allow(dead_code)] //will build this out at a later time
    pub paper_size: String,
    pub is_flipped: bool,
    pub columns: usize,
    pub(super) text_color_picker: Entity<ColorPickerState>,
    // Dropdown state
    pub font_families: Vec<String>,
    pub font_dropdown_open: bool,
    pub font_size_dropdown_open: bool,
}

impl RibbonPanel {
    pub fn new(font_families: Vec<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let text_color_picker = cx.new(|cx| {
            ColorPickerState::new(window, cx).default_value(cx.theme().colors.foreground) // Match active theme text color
        });

        // Subscribe to color selection events
        cx.subscribe(&text_color_picker, |this, _, event, cx| {
            if let ColorPickerEvent::Change(Some(color)) = event {
                this.emit_text_color(*color, cx);
            }
        })
        .detach();

        Self {
            active_tab: RibbonTab::Home,
            selected_font: "Liberation Sans".to_string(),
            original_font: None,
            font_size: 11.0,
            original_font_size: None,
            is_bold: false,
            is_italic: false,
            is_underline: false,
            paper_size: "us-letter".to_string(),
            is_flipped: false,
            columns: 1,
            text_color_picker,
            font_families,
            font_dropdown_open: false,
            font_size_dropdown_open: false,
        }
    }

    fn select_tab(&mut self, tab: RibbonTab, cx: &mut Context<Self>) {
        self.active_tab = tab;
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

    pub fn emit_text_color(&self, color: Hsla, cx: &mut Context<Self>) {
        let rgba = color.to_rgb();
        let hex_color = format!(
            "rgb(\"#{:02x}{:02x}{:02x}\")",
            (rgba.r * 255.0).round() as u8,
            (rgba.g * 255.0).round() as u8,
            (rgba.b * 255.0).round() as u8
        );
        cx.emit(RibbonAction::SetTextColor(hex_color));
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
            .hover(|style| style.bg(colors.primary_hover))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, on_click)
            .child(label)
    }
}

impl RibbonPanel {
    pub fn update_active_properties(
        &mut self,
        props: &ActiveProperties,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut changed = false;

        if self.is_bold != props.is_bold {
            self.is_bold = props.is_bold;
            changed = true
        }

        if self.is_italic != props.is_italic {
            self.is_italic = props.is_italic;
            changed = true;
        }

        if self.is_underline != props.is_underline {
            self.is_underline = props.is_underline;
            changed = true;
        }

        if let Some(ref font) = props.font {
            if &self.selected_font != font {
                self.selected_font = font.clone();
                changed = true;
            }
        }

        if let Some(size) = props.size {
            if (self.font_size - size).abs() > 0.01 {
                self.font_size = size;
                changed = true;
            }
        }

        if let Some(ref color_str) = props.color {
            if let Some(hsla) = parse_typst_color_to_hsla(color_str) {
                let picker = self.text_color_picker.clone();
                picker.update(cx, |state, picker_cx| {
                    state.set_value(hsla, _window, picker_cx);
                });
            }
        }

        if changed {
            cx.notify();
        }
    }
}

fn parse_typst_color_to_hsla(s: &str) -> Option<Hsla> {
    let trimmed = s.trim().trim_matches('"');

    match trimmed.to_lowercase().as_str() {
        "black" => return Some(black()),
        "white" => return Some(white()),
        "red" => return Some(rgb(0xff0000).into()),
        "green" => return Some(rgb(0x00ff00).into()),
        "blue" => return Some(rgb(0x0000ff).into()),
        "yellow" => return Some(rgb(0xffff00).into()),
        "cyan" => return Some(rgb(0x00ffff).into()),
        "magenta" => return Some(rgb(0xff00ff).into()),
        "gray" | "grey" => return Some(rgb(0x808080).into()),
        _ => {}
    }

    let hex_candidate = if let Some(inner) = trimmed
        .strip_prefix("rgb(")
        .and_then(|s| s.strip_suffix(')'))
    {
        inner.trim().trim_matches('"').trim_matches('\'')
    } else {
        trimmed
    };

    if let Some(hex) = hex_candidate.strip_prefix('#') {
        if hex.len() == 6 {
            if let Ok(val) = u32::from_str_radix(hex, 16) {
                return Some(rgb(val).into());
            }
        } else if hex.len() == 3 {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            return Some(
                Rgba {
                    r: r as f32 / 255.0,
                    g: g as f32 / 255.0,
                    b: b as f32 / 255.0,
                    a: 1.0,
                }
                .into(),
            );
        }
    }

    None
}
