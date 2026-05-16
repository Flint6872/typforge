use crate::workspace::TypstNoteView;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{ActiveTheme, button::Button, h_flex};

impl<W: typst_gpui::TypstGpuiWorld> Render for TypstNoteView<W> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        //let is_maximized = window.is_fullscreen();

        // 1. Root Container (Vertical Flex)
        div()
            .flex_col()
            .size_full()
            //.track_focus(&cx.focus_handle())
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
            //zoom in
            .on_action(
                cx.listener(|this, _action: &crate::actions::ZoomIn, window, cx| {
                    // Use 'window' from the closure arguments, not the render method
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
                    // Use 'window' from the closure arguments
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
            //.child(self.dock_area.clone())
            .child(
                // 2. Title Bar
                div()
                    .h_8()
                    .w_full()
                    // .bg(rgb(0x323232))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .px_4()
                            .flex_grow()
                            // This makes the entire title bar draggable
                            .window_control_area(WindowControlArea::Drag)
                            .child("TypForge"),
                    ) //add menu bar if not on macOS
                    .child(
                        h_flex()
                            .items_center()
                            .child(Button::new("win-min").label("-").on_click(|_, window, cx| {
                                println!("Minimize clicked");
                                window.minimize_window();
                                cx.stop_propagation();
                            }))
                            .child(
                                Button::new("win-max")
                                    // In modern GPUI, use is_zoomed() to check maximization
                                    .label(if window.is_fullscreen() { "❐" } else { "□" })
                                    .on_click(|_, window, cx| {
                                        println!("Zoom (Maximize) clicked");
                                        window.toggle_fullscreen();
                                        cx.stop_propagation();
                                    }),
                            )
                            .child(Button::new("win-close").label("×").on_click(
                                |_, window, cx| {
                                    println!("Close clicked");
                                    window.remove_window();
                                    cx.stop_propagation();
                                },
                            )),
                    )
                    .flex_none(),
            )
            .when_some(self.menu_bar.clone(), |this, menu_bar| {
                this.child(
                    div()
                        // .bg(rgb(0x252525))
                        .border_b_1()
                        // .border_color(rgb(0x3c3c3c))
                        // Now menu_bar is the unwrapped Entity, which implements IntoElement
                        .child(menu_bar),
                )
            })
            .child(
                // 2. Main Body Area (Horizontal Flex) - Now handled by DockArea
                self.dock_area.clone(),
            )
    }
}
