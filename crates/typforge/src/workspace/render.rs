use crate::{
    actions::{self},
    workspace::TypstNoteView,
};
use gpui::Focusable;
use gpui::InteractiveElement;
use gpui::prelude::FluentBuilder;
use gpui::{
    Context, Element, IntoElement, MouseButton, ParentElement, Render, StatefulInteractiveElement,
    Styled, Window, deferred, div, px, rgb,
};
use gpui_component::ActiveTheme;
use gpui_component::scroll::ScrollableElement;
//use gpui_component::dock::PanelView;

impl<W: typst_gpui::TypstGpuiWorld + typastry::IdeWorld + 'static> Render for TypstNoteView<W> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 1. Root Container (Vertical Flex)
        let mut root = div()
            .flex_col()
            .flex_row()
            .size_full()
            .bg(cx.theme().background) // Dark background
            // --- FileNew Action ---
            .on_action(cx.listener(Self::handle_file_new))
            // --- FileOpen Action ---
            .on_action(cx.listener(Self::handle_file_open))
            // --- FolderOpen Action ---
            .on_action(cx.listener(Self::handle_folder_open))
            // --- FileSaveAs Action ---
            .on_action(cx.listener(Self::handle_file_save_as))
            // --- FileClose Action ---
            .on_action(cx.listener(Self::handle_file_close))
            // --- FileQuit Action ---
            .on_action(cx.listener(Self::handle_file_quit))
            //file save
            .on_action(cx.listener(Self::handle_file_save))
            // --- FileOpenRecent Action (Ctrl+R) ---
            .on_action(cx.listener(Self::handle_file_open_recent))
            // --- OpenRecentFiles Action (Selecting a file) ---
            .on_action(cx.listener(Self::handle_open_specific_recent_file))
            //zoom in
            .on_action(
                cx.listener(|this, _action: &crate::actions::ZoomIn, window, cx| {
                    if this
                        .editor_panel
                        .focus_handle(cx)
                        .contains_focused(window, cx)
                    {
                        this.editor_panel.update(cx, |editor, cx| {
                            editor.set_zoom(editor.zoom_level + 0.1, cx);
                        });
                    } else if this
                        .preview_panel
                        .focus_handle(cx)
                        .contains_focused(window, cx)
                    {
                        this.preview_panel.update(cx, |preview, cx| {
                            preview.zoom_in(cx);
                        });
                    }
                }),
            )
            // zoom out
            .on_action(
                cx.listener(|this, _action: &crate::actions::ZoomOut, window, cx| {
                    if this
                        .editor_panel
                        .focus_handle(cx)
                        .contains_focused(window, cx)
                    {
                        this.editor_panel.update(cx, |editor, cx| {
                            editor.set_zoom(editor.zoom_level - 0.1, cx);
                        });
                    } else if this
                        .preview_panel
                        .focus_handle(cx)
                        .contains_focused(window, cx)
                    {
                        this.preview_panel.update(cx, |preview, cx| {
                            preview.zoom_out(cx);
                        });
                    }
                }),
            )
            //zoom reset
            .on_action(
                cx.listener(|this, _action: &crate::actions::ResetZoom, window, cx| {
                    if this
                        .editor_panel
                        .focus_handle(cx)
                        .contains_focused(window, cx)
                    {
                        this.editor_panel.update(cx, |editor, cx| {
                            editor.set_zoom(1.0, cx);
                        });
                    } else if this
                        .preview_panel
                        .focus_handle(cx)
                        .contains_focused(window, cx)
                    {
                        this.preview_panel.update(cx, |preview, cx| {
                            preview.set_zoom(1.0, cx);
                        });
                    }
                }),
            )
            //export to pdf
            .on_action(cx.listener(Self::handle_export_pdf))
            //export to docx
            .on_action(cx.listener(Self::handle_export_docx))
            //reload settings
            .on_action(cx.listener(Self::handle_reload_settings))
            .on_action(cx.listener(|this, action: &actions::ToggleBold, _, cx| {
                this.ribbon_panel
                    .update(cx, |ribbon, cx| ribbon.handle_toggle_bold(action, cx));
            }))
            .on_action(cx.listener(|this, action: &actions::ToggleItalic, _, cx| {
                this.ribbon_panel
                    .update(cx, |ribbon, cx| ribbon.handle_toggle_italic(action, cx));
            }))
            .on_action(
                cx.listener(|this, action: &actions::ToggleUnderline, _, cx| {
                    this.ribbon_panel
                        .update(cx, |ribbon, cx| ribbon.handle_toggle_underline(action, cx));
                }),
            )
            .when_some(self.menu_bar.clone(), |this, menu_bar| {
                this.child(div().border_b_1().child(menu_bar))
            })
            // --- 3. Render Ribbon Panel ---
            .child(self.ribbon_panel.clone())
            .child(div().flex_grow().h_5_6().child(self.dock_area.clone()))
            .child(
                div()
                    .w_full()
                    .h_8()
                    .bg(cx.theme().foreground)
                    .text_color(cx.theme().foreground)
                    .p_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .children(vec![gpui::div().child("Status: Ready")]),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .children(vec![gpui::div().child("Line: 1, Col: 1")]),
                    ),
            );

        // --- 4. Conditionally render picker as overlay ---
        if self.show_recent_files_picker {
            root = root.child(render_recent_files_picker(self, window, cx));
        }

        root
    }
}

/// Renders the modal picker UI for recent files.
pub fn render_recent_files_picker<W: typst_gpui::TypstGpuiWorld + typastry::IdeWorld + 'static>(
    this: &mut TypstNoteView<W>,
    _window: &mut Window,
    cx: &mut Context<TypstNoteView<W>>,
) -> impl IntoElement {
    let settings = cx.global::<crate::settings::AppSettings>();
    let recent_files = settings.recent_files.clone();

    // Use `deferred()` to lift the modal into a separate rendering pass.
    deferred(
        div()
            .size_full()
            .absolute()
            .top_0()
            .left_0()
            .flex()
            .justify_center()
            // .items_center()
            // Semi-transparent black backdrop overlay using hex rgba (b0 is ~70% opacity)
            .bg(gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.7,
            })
            // Dismiss picker when pressing Escape
            .on_action(cx.listener(|this, _: &actions::Dismiss, _window, cx| {
                this.show_recent_files_picker = false;
                cx.notify();
            }))
            // Close picker when clicking the background overlay
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    this.show_recent_files_picker = false;
                    cx.notify();
                }),
            )
            .child(
                // Centered modal dialog box
                div()
                    .w(px(500.0))
                    .max_h(px(400.0))
                    .bg(rgb(0x1e1e24)) // Dark background matching the editor
                    .rounded_lg()
                    .p_4()
                    .border_1()
                    .border_color(rgb(0x3e3e4a))
                    .shadow_lg()
                    .track_focus(&this.recent_picker_focus_handle)
                    // Prevent clicks inside the dialog from bubbling to the backdrop
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0x808080))
                                    .pb_2()
                                    .border_b_1()
                                    .border_color(rgb(0x3e3e4a))
                                    .child("Recent Files"),
                            )
                            .child(
                                // Scrollable list of items
                                div()
                                    .flex()
                                    .flex_col()
                                    .overflow_y_hidden()
                                    .gap_1()
                                    .children(recent_files.into_iter().enumerate().map(
                                        |(ix, path)| {
                                            let path_buf = std::path::PathBuf::from(&path);
                                            let file_name = path_buf
                                                .file_name()
                                                .map(|n| n.to_string_lossy().to_string())
                                                .unwrap_or_else(|| path.clone());

                                            let path_clone = path.clone();

                                            let is_selected =
                                                this.recent_picker_selected_index == Some(ix);

                                            div()
                                                .id(ix) // <--- CRITICAL: Gives the div an ID, enabling interactive listeners
                                                .p_2()
                                                .rounded_md()
                                                .flex()
                                                .flex_col()
                                                .cursor_pointer()
                                                // Conditional selection backgrounds
                                                .when(is_selected, |style| style.bg(rgb(0x2d3139)))
                                                .when(!is_selected, |style| {
                                                    style.hover(|hover_style| {
                                                        hover_style.bg(rgb(0x252931))
                                                    })
                                                })
                                                .on_click(cx.listener(
                                                    move |this, _, window, cx| {
                                                        this.handle_open_specific_recent_file(
                                                            &actions::OpenRecentFiles {
                                                                path: path_clone.clone(),
                                                            },
                                                            window,
                                                            cx,
                                                        );
                                                    },
                                                ))
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .text_color(rgb(0xffffff))
                                                        .child(file_name),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(rgb(0x808080))
                                                        .child(path),
                                                )
                                        },
                                    )),
                            ),
                    ),
            ),
    )
}
