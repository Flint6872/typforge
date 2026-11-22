// crates/typforge/src/editor/editor_panel/render.rs

use crate::editor::{CodeEditor, DraggedTab, EditorPanel, FileContentUpdated, TabDrag};
use gpui_kit::component::{ActiveTheme, StyledExt, h_flex, v_flex};
use gpui_kit::*;

use gpui_kit::prelude::FluentBuilder;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;
use typastry::intel::Tooltip;

impl<W: typst::World + typastry::IdeWorld + typst_gpui::TypstGpuiWorld + 'static> Render
    for EditorPanel<W>
{
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_file_path = self.active_file_path.clone();

        let font_size = px(16.0 * self.zoom_level);
        let line_height = font_size * 1.5;

        #[cfg(not(target_os = "macos"))]
        let ctrl_or_cmd = "Ctrl";

        #[cfg(target_os = "macos")]
        let ctrl_or_cmd = "Cmd";

        // 1. Create the base Editor UI
        let editor_element = if self.open_files.is_empty() {
            div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .text_color(cx.theme().foreground)
                .child(
                    div()
                        .text_xl()
                        .font_weight(FontWeight(2.0))
                        .child("No Document Open"),
                )
                .child(
                    div()
                        .text_lg()
                        .margins(px(8.))
                        .child("Create a new file or open an existing project to get started."),
                )
                .child(div().flex_row().gap_4().margins(px(16.)).children(vec![
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .gap_4()
                                .child(div().text_lg().font_bold().child("New File"))
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap_1()
                                        .child(div().child(ctrl_or_cmd))
                                        .child(div().child("+"))
                                        .child(div().child("N")),
                                ),
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .gap_4()
                                .child(div().text_lg().font_bold().child("Open File"))
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap_1()
                                        .child(div().child(ctrl_or_cmd))
                                        .child(div().child("+"))
                                        .child(div().child("O")),
                                ),
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .gap_4()
                                .child(div().text_lg().font_bold().child("Open Folder"))
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap_1()
                                        .child(div().child(ctrl_or_cmd))
                                        .child(div().child("+"))
                                        .child(div().child("K")),
                                ),
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .gap_4()
                                .child(div().text_lg().font_bold().child("Open Recent"))
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap_1()
                                        .child(div().child(ctrl_or_cmd))
                                        .child(div().child("+"))
                                        .child(div().child("R")),
                                ),
                        ]))
                .into_any_element()
        } else if let Some(ref active_path) = active_file_path {
            if active_path == &PathBuf::from("typforge://package-manager") {
                if let Some(pkg_view) = &self.package_manager_view {
                    pkg_view.clone().into_any_element()
                } else {
                    div().child("Loading Package Manager...").into_any_element()
                }
            } else if let Some(file) = self.open_files.iter().find(|f| f.path == *active_path) {
                let active_path = active_path.clone();

                CodeEditor::new(file.editor_state.clone(), file.language.clone(), Vec::new())
                    .font_size(font_size)
                    .line_height(line_height)
                    .h_full()
                    .on_mouse_move(cx.listener(
                        move |this_entity, event: &MouseMoveEvent, _, cx| {
                            let now = Instant::now();
                            if this_entity.last_hover_request_time.map_or(true, |t| {
                                now.duration_since(t) > Duration::from_millis(500)
                            }) {
                                this_entity.last_hover_request_time = Some(now);
                                let Some(active_file) = this_entity
                                    .open_files
                                    .iter()
                                    .find(|f| f.path == active_path)
                                else {
                                    return;
                                };

                                let code_editor_entity = active_file.code_editor_entity.clone();
                                let Some(byte_offset) = code_editor_entity
                                    .read(cx)
                                    .screen_to_byte_offset(event.position, cx)
                                else {
                                    return;
                                };

                                this_entity.request_hover(byte_offset, event.position, cx);
                            }
                        },
                    ))
                    .on_mouse_down(cx.listener(|this, _, _, cx| {
                        this.clear_hover(cx);
                    }))
                    .into_any_element()
            } else {
                div().into_any_element()
            }
        } else {
            div().into_any_element()
        };

        // 2. Prepare the Hover Popup if it exists
        let hover_popup = self.current_hover_content.as_ref().map(|hover| {
            let pos = self.current_hover_position.unwrap_or_default();
            let text = match hover {
                Tooltip::Text(text) => text.to_string(),
                Tooltip::Code(code) => format!("```typst\n{}\n```", code),
            };

            div()
                .absolute()
                .top(pos.y + px(20.))
                .left(pos.x)
                .bg(cx.theme().colors.popover)
                .border_1()
                .border_color(cx.theme().colors.border)
                .p_2()
                .rounded_md()
                .shadow_lg()
                .child(text)
        });

        // 3. Compute real-time cursor screen position from active editor state
        let cursor_screen_pos = active_file_path.as_ref().and_then(|active_path| {
            let file = self.open_files.iter().find(|f| f.path == *active_path)?;
            let state = file.editor_state.read(cx);
            let cursor = state.cursor();
            let len = state.text().len();
            let range = if cursor < len {
                cursor..cursor + 1
            } else if cursor > 0 {
                cursor - 1..cursor
            } else {
                0..0
            };
            let bounds = state.range_to_bounds(&range)?;
            Some(point(bounds.left(), bounds.bottom()))
        });

        // 4. Prepare Signature Help Popup at the exact cursor position
        let signature_help_popup = self.current_signature_help.as_ref().and_then(|help| {
            let pos = cursor_screen_pos?;
            let active_idx = help.active_parameter;

            let params_element =
                h_flex()
                    .gap_1()
                    .flex_wrap()
                    .children(help.parameters.iter().enumerate().map(|(idx, p)| {
                        let is_active = idx == active_idx;
                        div()
                            .text_sm()
                            .when(is_active, |s| s.font_bold().text_color(rgb(0x61afef)))
                            .child(format!(
                                "{}{}",
                                p.name,
                                if idx + 1 < help.parameters.len() {
                                    ", "
                                } else {
                                    ""
                                }
                            ))
                    }));

            // Extract just the summary paragraph (first block) rather than the whole manual
            let summary_doc = help.doc.as_ref().and_then(|doc_text| {
                let first_para: String = doc_text
                    .lines()
                    .take_while(|line| !line.trim().is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                (!first_para.trim().is_empty()).then_some(first_para)
            });

            Some(
                anchored()
                    .position(point(pos.x, pos.y + px(4.0)))
                    .snap_to_window()
                    .child(
                        v_flex()
                            .max_w(px(550.0))
                            .max_h(px(220.0))
                            .bg(cx.theme().colors.popover)
                            .border_1()
                            .border_color(cx.theme().colors.border)
                            .p_2()
                            .rounded_md()
                            .shadow_lg()
                            .child(
                                h_flex()
                                    .flex_wrap()
                                    .text_color(cx.theme().colors.foreground)
                                    .child(format!("{}(", help.func_name))
                                    .child(params_element)
                                    .child(")"),
                            )
                            .when_some(summary_doc, |s, doc_text| {
                                s.child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().colors.muted_foreground)
                                        .px_1()
                                        .mt_1()
                                        .child(doc_text),
                                )
                            }),
                    ),
            )
        });

        // 5. Stack the editor and the popups
        div()
            .size_full()
            .track_focus(&self.focus_handle)
            .on_scroll(cx.listener(|this, event: &gpui::ScrollEvent, _win, cx| {
                if event.modifiers.control || event.modifiers.platform {
                    let delta = event.delta.pixel_delta(px(1.0)).y;
                    if delta > px(0.) {
                        this.set_zoom(this.zoom_level + 0.1, cx);
                    } else if delta < px(0.) {
                        this.set_zoom(this.zoom_level - 0.1, cx);
                    }
                }
            }))
            .flex_col()
            .child(
                div().w_full().bg(cx.theme().tab_bar).child(
                    h_flex().flex_nowrap().items_baseline().children(
                        if self.open_files.is_empty() {
                            vec![
                                div()
                                    .px_4()
                                    .text_color(cx.theme().background)
                                    .child("No files open")
                                    .into_any_element(),
                            ]
                        } else {
                            self.open_files
                                .iter()
                                .enumerate()
                                .map(|(ix, f)| {
                                    let is_active = Some(&f.path) == active_file_path.as_ref();
                                    let path = f.path.clone();

                                    div()
                                        .flex_shrink_0()
                                        .bg(cx.theme().tab_bar)
                                        .text_color(if is_active {
                                            cx.theme().tab_foreground
                                        } else {
                                            cx.theme().background
                                        })
                                        .id(("tab", ix))
                                        .flex()
                                        .items_baseline()
                                        .px_3()
                                        .cursor_pointer()
                                        .bg(if is_active {
                                            cx.theme().tab_active
                                        } else {
                                            cx.theme().tab_foreground
                                        })
                                        .border_r_1()
                                        .border_color(cx.theme().tab_bar_segmented)
                                        .on_click(cx.listener({
                                            let path = path.clone();
                                            move |this, _, _win, cx| {
                                                this.active_file_path = Some(path.clone());
                                                cx.notify();
                                                if let Some(active_file_index) = this
                                                    .open_files
                                                    .iter()
                                                    .position(|f| f.path == path)
                                                {
                                                    let content = this.open_files
                                                        [active_file_index]
                                                        .editor_state
                                                        .read(cx)
                                                        .text()
                                                        .to_string();
                                                    cx.emit(FileContentUpdated {
                                                        path: Some(path.clone()),
                                                        content,
                                                    });
                                                }
                                            }
                                        }))
                                        .on_drag(TabDrag { from_index: ix }, {
                                            let tab_name = f.tab_name();
                                            move |_drag, _point, _window, cx| {
                                                cx.new(|_| DraggedTab {
                                                    name: tab_name.clone(),
                                                })
                                            }
                                        })
                                        .on_drop(cx.listener(
                                            move |this, drag: &TabDrag, _win, cx| {
                                                this.move_tab(drag.from_index, ix, cx);
                                            },
                                        ))
                                        .child(f.tab_name())
                                        .child(if !f.diagnostics.is_empty() {
                                            div()
                                                .ml_1()
                                                .w_2()
                                                .h_2()
                                                .rounded_full()
                                                .bg(rgb(0xFF0000))
                                                .into_any_element()
                                        } else {
                                            div().into_any_element()
                                        })
                                        .child(
                                            div()
                                                .id(("close", ix))
                                                .hover(|s| {
                                                    s.bg(cx.theme().button_primary_hover)
                                                        .rounded_sm()
                                                })
                                                .child(" X")
                                                .on_click(cx.listener(move |this, _, _win, cx| {
                                                    this.close_file(path.clone(), cx);
                                                })),
                                        )
                                        .into_any_element()
                                })
                                .collect::<Vec<_>>()
                        },
                    ),
                ),
            )
            .child(
                div()
                    .flex_grow_1()
                    .h_4_5()
                    .flex_col()
                    .child(editor_element)
                    .children(hover_popup)
                    .children(signature_help_popup),
            )
    }
}
