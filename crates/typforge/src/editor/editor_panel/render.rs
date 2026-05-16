use crate::editor::{CodeEditor, DraggedTab, EditorPanel, FileContentUpdated, TabDrag};
use gpui::*;
use gpui_component::{ActiveTheme, StyledExt, h_flex};

use std::time::Duration;
use tokio::time::Instant;
// Use types from typstography (0.94) for the backend communication
use typstography::HoverContents;

impl Render for EditorPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_file_path = self.active_file_path.clone();

        // 1. Create the base Editor UI
        let editor_element = if let Some(ref active_path) = active_file_path {
            if let Some(file) = self.open_files.iter().find(|f| f.path == *active_path) {
                let uri = file.uri();
                //let editor_panel_handle = cx.entity().clone();

                CodeEditor::new(file.editor_state.clone(), file.language.clone(), Vec::new())
                    .font_size(px(25.0 * self.zoom_level))
                    .line_height(px(35.0 * self.zoom_level))
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
                                    .find(|f| f.path == uri.to_file_path().unwrap())
                                else {
                                    return;
                                };

                                let code_editor_entity = active_file.code_editor_entity.clone();
                                let Some(lsp_position) = code_editor_entity
                                    .read(cx)
                                    .screen_to_lsp_position(event.position, cx)
                                else {
                                    return;
                                };

                                this_entity.request_hover(
                                    uri.clone(),
                                    lsp_position,
                                    event.position,
                                    cx,
                                );
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
            let text = match &hover.contents {
                HoverContents::Markup(m) => m.value.clone(),
                _ => "Hover info".to_string(),
            };

            div()
                .absolute()
                .top(pos.y + px(20.))
                .left(pos.x)
                //  .z_index(100)
                .bg(rgb(0x333333))
                .border_1()
                .border_color(rgb(0x555555))
                .p_2()
                .rounded_md()
                .shadow_lg()
                .child(text)
        });

        // 3. Stack the editor and the popup
        div()
            .size_full()
            //.bg(cx.theme().background)
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
                // Tab Bar
                h_flex()
                    .items_baseline()
                    .v_flex()
                    .bg(cx.theme().foreground)
                    .child(if self.open_files.is_empty() {
                        div()
                            .px_4()
                            .text_color(cx.theme().background)
                            .child("No files open")
                    } else {
                        h_flex().children(self.open_files.iter().enumerate().map(|(ix, f)| {
                            let is_active = Some(&f.path) == active_file_path.as_ref();
                            let path = f.path.clone();

                            div()
                                .bg(cx.theme().tab_bar) //tab color
                                .text_color(if is_active {
                                    cx.theme().tab_foreground
                                } else {
                                    cx.theme().background
                                }) //tab text color
                                .id(("tab", ix))
                                .flex()
                                .items_baseline()
                                //.gap_2()
                                .px_3()
                                //.h_full()
                                .cursor_pointer()
                                .bg(if is_active {
                                    cx.theme().tab_active
                                } else {
                                    cx.theme().tab_foreground
                                })
                                .border_r_1()
                                .border_color(cx.theme().tab_bar_segmented)
                                // 1. Click to Switch
                                .on_click(cx.listener({
                                    let path = path.clone();
                                    move |this, _, _win, cx| {
                                        this.active_file_path = Some(path.clone());
                                        cx.notify(); // Notify UI about tab change

                                        // ADD THIS: Emit content update when tab is selected
                                        if let Some(active_file_index) =
                                            this.open_files.iter().position(|f| f.path == path)
                                        {
                                            let active_file = &this.open_files[active_file_index];
                                            let content = active_file
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
                                // 2. Drag to Reorder
                                .on_drag(TabDrag { from_index: ix }, {
                                    let tab_name = f.tab_name();
                                    // Closure now takes 4 arguments and returns an Entity
                                    move |_drag, _point, _window, cx| {
                                        cx.new(|_| DraggedTab {
                                            name: tab_name.clone(),
                                        })
                                    }
                                })
                                .on_drop(cx.listener(move |this, drag: &TabDrag, _win, cx| {
                                    this.move_tab(drag.from_index, ix, cx);
                                }))
                                .child(f.tab_name())
                                // Display an error indicator on the tab if there are diagnostics
                                .child(if !f.diagnostics.is_empty() {
                                    div()
                                        .ml_1()
                                        .w_2()
                                        .h_2()
                                        .rounded_full()
                                        .bg(rgb(0xFF0000)) // Red dot for errors
                                        .into_any_element()
                                } else {
                                    div().into_any_element()
                                })
                                // 3. Close Button
                                .child(
                                    div()
                                        .id(("close", ix))
                                        .hover(|s| {
                                            s.bg(cx.theme().button_primary_hover).rounded_sm()
                                        })
                                        .child(" X")
                                        //.child(IconName::Close)
                                        .on_click(cx.listener(move |this, _, _win, cx| {
                                            this.close_file(path.clone(), cx);
                                        })),
                                )
                        }))
                    }),
            )
            .child(
                div()
                    .flex_grow()
                    .size_full()
                    .flex_col()
                    .child(editor_element)
                    .children(hover_popup),
            )
    }
}
