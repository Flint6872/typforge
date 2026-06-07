// crates/typforge/src/editor/editor_panel/render.rs

use crate::editor::{CodeEditor, DraggedTab, EditorPanel, FileContentUpdated, TabDrag};
use gpui::*;
use gpui_component::popover::Popover;
use gpui_component::{ActiveTheme, StyledExt, h_flex, scroll::ScrollableElement};

use std::time::Duration;
use std::time::Instant;
use typforge_core::intel::Tooltip;

impl<W: typst::World + typforge_core::IdeWorld + typst_gpui::TypstGpuiWorld + 'static> Render
    for EditorPanel<W>
{
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_file_path = self.active_file_path.clone();

        let font_size = px(16.0 * self.zoom_level);
        let line_height = font_size * 1.5;

        // 1. Create the base Editor UI
        let editor_element = if let Some(ref active_path) = active_file_path {
            if let Some(file) = self.open_files.iter().find(|f| f.path == *active_path) {
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
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child("No files open")
                .into_any_element()
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
                .bg(rgb(0x333333))
                .border_1()
                .border_color(rgb(0x555555))
                .p_2()
                .rounded_md()
                .shadow_lg()
                .child(text)
        });

        // 3. Prepare the completions autocomplete popup menu if active
        let completions_popup = if !self.completions.is_empty() {
            if let (Some(active_path), Some(start_idx)) =
                (&active_file_path, self.completions_trigger_index)
            {
                if let Some(file) = self.open_files.iter().find(|f| f.path == *active_path) {
                    let editor_state = file.editor_state.read(cx);
                    let cursor = editor_state.cursor();

                    let origin_bounds = editor_state.range_to_bounds(&(0..0));
                    let char_range = start_idx..(start_idx + 1); // Position of '#'
                    let char_bounds = editor_state.range_to_bounds(&char_range);

                    if let (Some(ob), Some(cb)) = (origin_bounds, char_bounds) {
                        let local_x = cb.left() - ob.left() + px(120.0);
                        let local_y = cb.bottom() - ob.top() + px(50.0);

                        let file_state = file.editor_state.clone();
                        let content = editor_state.text().to_string();

                        // Fetch the prefix currently typed after the '#'
                        let typed_prefix = if cursor > start_idx + 1 {
                            content[(start_idx + 1)..cursor].to_lowercase()
                        } else {
                            String::new()
                        };

                        // Filter completions based on prefix
                        let filtered_completions: Vec<_> = self
                            .completions
                            .iter()
                            .filter(|c| {
                                if typed_prefix.is_empty() {
                                    true
                                } else {
                                    c.label
                                        .to_string()
                                        .to_lowercase()
                                        .starts_with(&typed_prefix)
                                }
                            })
                            .collect();

                        let items: Vec<_> = filtered_completions
                            .iter()
                            .enumerate()
                            .map(|(idx, c)| {
                                let label = c.label.to_string();
                                let apply_text = c
                                    .apply
                                    .as_ref()
                                    .map(|s| s.to_string())
                                    .unwrap_or_else(|| label.clone());

                                div()
                                    .id(("completion-item", idx))
                                    .px_2()
                                    .py_1()
                                    .text_color(rgb(0xABB2BF))
                                    .hover(|s| s.bg(rgb(0x3E4452)))
                                    .cursor_pointer()
                                    .on_click(cx.listener({
                                        let apply_text = apply_text.clone();
                                        let file_state = file_state.clone();
                                        move |this, _, window, cx| {
                                            this.completions.clear();
                                            this.completions_trigger_index = None;

                                            file_state.update(cx, |state, input_cx| {
                                                let cursor = state.cursor();
                                                let replace_range = (start_idx + 1)..cursor;
                                                state.replace_range_with_history(
                                                    replace_range,
                                                    &apply_text,
                                                    window,
                                                    input_cx,
                                                );
                                                state.focus(window, input_cx);
                                            });

                                            cx.notify();
                                        }
                                    }))
                                    .child(label)
                            })
                            .collect();

                        if !items.is_empty() {
                            Some(
                                div()
                                    .absolute()
                                    .top(local_y)
                                    .left(local_x)
                                    .bg(rgb(0x282C34))
                                    .border_1()
                                    .border_color(rgb(0x3E4452))
                                    .p_1()
                                    .rounded_md()
                                    .shadow_lg()
                                    .w(px(220.0))
                                    .max_h(px(250.0)) // Explicit outer height to satisfy GPUI layout
                                    .flex()
                                    .flex_col()
                                    .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                        this.completions.clear();
                                        this.completions_trigger_index = None;
                                        cx.notify();
                                    }))
                                    .child(
                                        // Scrollable inner container that handles overflow gracefully
                                        div().flex_grow().overflow_y_scrollbar().children(items),
                                    ),
                            )
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        // 3. Stack the editor and the popup
        div()
            .size_full()
            .track_focus(&self.focus_handle)
            .on_scroll_wheel(
                cx.listener(|this, event: &gpui::ScrollWheelEvent, _win, cx| {
                    if event.modifiers.control || event.modifiers.platform {
                        let delta = event.delta.pixel_delta(px(1.0)).y;
                        if delta > px(0.) {
                            this.set_zoom(this.zoom_level + 0.1, cx);
                        } else if delta < px(0.) {
                            this.set_zoom(this.zoom_level - 0.1, cx);
                        }
                    }
                }),
            )
            .flex_col()
            .child(
                // Scrollable Tab Bar Container
                div().w_full().bg(cx.theme().foreground).child(
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
                    .flex_grow()
                    .size_full()
                    .flex_col()
                    .child(editor_element)
                    .children(hover_popup)
                    .children(completions_popup),
            )
    }
}
