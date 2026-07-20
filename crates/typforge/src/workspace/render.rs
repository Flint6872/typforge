use crate::{
    actions::{self},
    workspace::TypstNoteView,
};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::ActiveTheme;

impl<W: typst_gpui::TypstGpuiWorld + typforge_core::IdeWorld + 'static> Render
    for TypstNoteView<W>
{
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        //let is_maximized = window.is_fullscreen();

        // 1. Root Container (Vertical Flex)
        div()
            .flex_col()
            .flex_row()
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
                this.child(
                    div()
                        // .bg(rgb(0x252525))
                        .border_b_1()
                        // .border_color(rgb(0x3c3c3c))
                        // Now menu_bar is the unwrapped Entity, which implements IntoElement
                        .child(menu_bar),
                )
            })
            // --- 3. Render Ribbon Panel ---
            // Positioned cleanly underneath the window menu bar, stretching full-width.
            .child(self.ribbon_panel.clone())
            .child(div().flex_grow().h_5_6().child(self.dock_area.clone()))
            .child(
                div()
                    .w_full()
                    .h_8() // Increase height to something visible, e.g., h_8 (32px)
                    .bg(cx.theme().foreground) // Give it a background color, slightly different from main background
                    .text_color(cx.theme().foreground) // Set default text color for content
                    .p_2() // Add some padding
                    .flex() // Use flexbox to arrange internal items
                    .items_center() // Vertically center items
                    .justify_between() // Distribute space between items (e.g., left and right aligned groups)
                    .child(
                        // Example left-aligned group
                        div().flex().gap_2().children(vec![
                            // Your buttons or text here
                            gpui::div().child("Status: Ready"),
                            // gpui::button("Save").on_click(...),
                        ]),
                    )
                    .child(
                        // Example right-aligned group
                        div().flex().gap_2().children(vec![
                            gpui::div().child("Line: 1, Col: 1"),
                            // gpui::button("Export").on_click(...),
                        ]),
                    ),
            )
    }
}
