use crate::{
    actions::{self},
    workspace::TypstNoteView,
};
use gpui_kit::component::ActiveTheme;
use gpui_kit::{
    Context, Focusable, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels,
    Render, StatefulInteractiveElement, Styled, Window, deferred, div, prelude::FluentBuilder, px,
    rgb,
};

/// Screen class breakpoints for responsive layouts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenClass {
    /// Mobile phones (< 640px)
    Compact,
    /// Tablets / small foldables (640px - 1024px)
    Medium,
    /// Desktops / large displays (> 1024px)
    Expanded,
}

impl ScreenClass {
    pub fn from_width(width: Pixels) -> Self {
        if width < px(640.0) {
            Self::Compact
        } else if width < px(1024.0) {
            Self::Medium
        } else {
            Self::Expanded
        }
    }

    pub fn is_compact(self) -> bool {
        self == Self::Compact
    }
}

impl<W: typst_gpui::TypstGpuiWorld + typastry::IdeWorld + 'static> Render for TypstNoteView<W> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let screen = ScreenClass::from_width(window.viewport_size().width);

        // Reconfigure dock layout dynamically when window size crosses breakpoints
        if screen != self.current_screen_class {
            self.update_dock_layout(window, cx);
        }

        // 1. Root Container (Clean vertical flex column)
        let mut root = div()
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background)
            // --- Actions ---
            .on_action(cx.listener(Self::handle_file_new))
            .on_action(cx.listener(Self::handle_file_open))
            .on_action(cx.listener(Self::handle_folder_open))
            .on_action(cx.listener(Self::handle_file_save_as))
            .on_action(cx.listener(Self::handle_file_close))
            .on_action(cx.listener(Self::handle_file_quit))
            .on_action(cx.listener(Self::handle_file_save))
            .on_action(cx.listener(Self::handle_file_open_recent))
            .on_action(cx.listener(Self::handle_open_specific_recent_file))
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
            .on_action(cx.listener(Self::handle_export_pdf))
            .on_action(cx.listener(Self::handle_export_docx))
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
            .on_action(cx.listener(Self::handle_package_manager));

        // 2. Top Chrome (Hide full desktop menu on compact screens)
        if !screen.is_compact() {
            if let Some(menu_bar) = self.menu_bar.clone() {
                root = root.child(div().w_full().border_b_1().child(menu_bar));
            }
            root = root.child(self.ribbon_panel.clone());
        } else {
            root = root.child(self.render_mobile_app_bar(window, cx));
        }

        // 3. Main Workspace Area (Consumes all available space)
        root = root.child(
            div()
                .flex_1()
                .w_full()
                .overflow_hidden()
                .child(self.dock_area.clone()),
        );

        // 4. Status Bar / Bottom Bar (Simplified on Compact)
        let status_bar = if screen.is_compact() {
            // Touch-friendly minimal height with essential indicator
            div()
                .w_full()
                .h(px(32.0))
                .bg(cx.theme().foreground)
                .text_color(cx.theme().foreground)
                .px_3()
                .flex()
                .items_center()
                .justify_between()
                .text_xs()
                .child("Ready")
                .child("Ln 1, Col 1")
        } else {
            div()
                .w_full()
                .h_8()
                .bg(cx.theme().foreground)
                .text_color(cx.theme().foreground)
                .px_4()
                .flex()
                .items_center()
                .justify_between()
                .child(div().flex().gap_2().child("Status: Ready"))
                .child(div().flex().gap_2().child("Line: 1, Col: 1"))
        };

        root = root.child(status_bar);

        // 5. Modals / Overlays
        if self.show_recent_files_picker {
            root = root.child(render_recent_files_picker(self, window, cx));
        }

        if self.show_mobile_menu {
            root = root.child(self.render_mobile_menu_overlay(window, cx));
        }

        root
    }
}

/// Renders the modal picker UI for recent files with responsive bounds.
pub fn render_recent_files_picker<W: typst_gpui::TypstGpuiWorld + typastry::IdeWorld + 'static>(
    this: &mut TypstNoteView<W>,
    _window: &mut Window,
    cx: &mut Context<TypstNoteView<W>>,
) -> impl IntoElement {
    let settings = cx.global::<crate::settings::AppSettings>();
    let recent_files = settings.recent_files.clone();

    deferred(
        div()
            .size_full()
            .absolute()
            .top_0()
            .left_0()
            .flex()
            .justify_center()
            .items_center()
            .p_4() // Keeps modal inset from screen edges on phones
            .bg(gpui_kit::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.7,
            })
            .on_action(cx.listener(|this, _: &actions::Dismiss, _window, cx| {
                this.show_recent_files_picker = false;
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    this.show_recent_files_picker = false;
                    cx.notify();
                }),
            )
            .child(
                // Responsive modal dialog box: clamps width rather than hardcoding it
                div()
                    .w_full()
                    .max_w(px(500.0))
                    .max_h(px(400.0))
                    .bg(rgb(0x1e1e24))
                    .rounded_lg()
                    .p_4()
                    .border_1()
                    .border_color(rgb(0x3e3e4a))
                    .shadow_lg()
                    .track_focus(&this.recent_picker_focus_handle)
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
                                                .id(ix)
                                                .p_2()
                                                .rounded_md()
                                                .flex()
                                                .flex_col()
                                                .cursor_pointer()
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
                                                        .truncate()
                                                        .child(path),
                                                )
                                        },
                                    )),
                            ),
                    ),
            ),
    )
}

impl<W: typst_gpui::TypstGpuiWorld + typastry::IdeWorld + 'static> TypstNoteView<W> {
    /// Renders a touch-friendly app bar for phone / compact viewports using the active theme.
    pub fn render_mobile_app_bar(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();

        let active_file_name = self
            .editor_panel
            .read(cx)
            .active_file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Typforge".to_string());

        let has_unsaved = self
            .editor_panel
            .read(cx)
            .active_file_path
            .as_ref()
            .and_then(|path| {
                self.editor_panel
                    .read(cx)
                    .open_files
                    .iter()
                    .find(|f| &f.path == path)
                    .map(|f| f.has_unsaved_changes)
            })
            .unwrap_or(false);

        let title_text = if has_unsaved {
            format!("* {}", active_file_name)
        } else {
            active_file_name
        };

        div()
            .w_full()
            .h(px(40.0))
            .bg(theme.title_bar)
            .border_b_1()
            .border_color(theme.border)
            .px_2()
            .flex()
            .items_center()
            .justify_between()
            .gap_1()
            // 1. Hamburger menu button (pinned left)
            .child(
                div()
                    .id("mobile-hamburger-btn")
                    .flex_shrink_0()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(theme.secondary)
                    .text_color(theme.foreground)
                    .border_1()
                    .border_color(theme.border)
                    .hover(|style| style.bg(theme.accent))
                    .cursor_pointer()
                    .text_xs()
                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.show_mobile_menu = !this.show_mobile_menu;
                        cx.notify();
                    }))
                    .child("☰ Menu"),
            )
            // 2. Active file title (flex_1 + shrinkable so it loses space first)
            .child(
                div()
                    .flex_1()
                    .flex_shrink_1()
                    .min_w_0()
                    .flex()
                    .justify_center()
                    .items_center()
                    .overflow_hidden()
                    .text_xs()
                    .font_weight(gpui_kit::FontWeight::BOLD)
                    .text_color(theme.foreground)
                    .truncate()
                    .child(title_text),
            )
            // 3. Quick action buttons (pinned right)
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .id("mobile-quick-save-btn")
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(theme.secondary)
                            .text_color(theme.foreground)
                            .border_1()
                            .border_color(theme.border)
                            .hover(|style| style.bg(theme.accent))
                            .cursor_pointer()
                            .text_xs()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.handle_file_save(&actions::FileSave, window, cx);
                            }))
                            .child("💾 Save"),
                    )
                    .child(
                        div()
                            .id("mobile-quick-recent-btn")
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(theme.secondary)
                            .text_color(theme.foreground)
                            .border_1()
                            .border_color(theme.border)
                            .hover(|style| style.bg(theme.accent))
                            .cursor_pointer()
                            .text_xs()
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.show_recent_files_picker = true;
                                cx.notify();
                            }))
                            .child("🕒 Recent"),
                    ),
            )
    }

    /// Renders a compact touch-friendly slide-over action drawer for mobile devices.
    pub fn render_mobile_menu_overlay(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();

        deferred(
            div()
                .size_full()
                .absolute()
                .top_0()
                .left_0()
                .flex()
                .flex_col()
                .justify_start()
                .bg(gpui_kit::Rgba {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.7,
                })
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _window, cx| {
                        this.show_mobile_menu = false;
                        cx.notify();
                    }),
                )
                .child(
                    div()
                        .w_full()
                        .max_w(px(280.0))
                        .max_h(px(520.0))
                        .bg(theme.popover)
                        .border_b_1()
                        .border_r_1()
                        .border_color(theme.border)
                        .shadow_lg()
                        .p_3()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        // Drawer Header
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .pb_2()
                                .border_b_1()
                                .border_color(theme.border)
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(gpui_kit::FontWeight::BOLD)
                                        .text_color(theme.popover_foreground)
                                        .child("Actions"),
                                )
                                .child(
                                    div()
                                        .id("mobile-menu-close-btn")
                                        .p_1()
                                        .cursor_pointer()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .hover(|style| style.text_color(theme.foreground))
                                        .on_click(cx.listener(|this, _, _window, cx| {
                                            this.show_mobile_menu = false;
                                            cx.notify();
                                        }))
                                        .child("✕ Close"),
                                ),
                        )
                        // Scrollable Item List
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .overflow_y_hidden()
                                .child(self.render_mobile_item(
                                    "item-new",
                                    "📄 New File",
                                    cx,
                                    |this, window, cx| {
                                        this.show_mobile_menu = false;
                                        this.handle_file_new(&actions::FileNew, window, cx);
                                    },
                                ))
                                .child(self.render_mobile_item(
                                    "item-open",
                                    "📂 Open File",
                                    cx,
                                    |this, window, cx| {
                                        this.show_mobile_menu = false;
                                        this.handle_file_open(&actions::FileOpen, window, cx);
                                    },
                                ))
                                .child(self.render_mobile_item(
                                    "item-folder",
                                    "📁 Open Folder",
                                    cx,
                                    |this, window, cx| {
                                        this.show_mobile_menu = false;
                                        this.handle_folder_open(&actions::FolderOpen, window, cx);
                                    },
                                ))
                                .child(self.render_mobile_item(
                                    "item-save",
                                    "💾 Save File",
                                    cx,
                                    |this, window, cx| {
                                        this.show_mobile_menu = false;
                                        this.handle_file_save(&actions::FileSave, window, cx);
                                    },
                                ))
                                .child(self.render_mobile_item(
                                    "item-save-as",
                                    "💾 Save As...",
                                    cx,
                                    |this, window, cx| {
                                        this.show_mobile_menu = false;
                                        this.handle_file_save_as(&actions::FileSaveAs, window, cx);
                                    },
                                ))
                                .child(self.render_mobile_item(
                                    "item-recent",
                                    "🕒 Recent Files",
                                    cx,
                                    |this, _window, cx| {
                                        this.show_mobile_menu = false;
                                        this.show_recent_files_picker = true;
                                        cx.notify();
                                    },
                                ))
                                .child(self.render_mobile_item(
                                    "item-pdf",
                                    "📕 Export PDF",
                                    cx,
                                    |this, window, cx| {
                                        this.show_mobile_menu = false;
                                        this.handle_export_pdf(&actions::FileExportPdf, window, cx);
                                    },
                                ))
                                .child(self.render_mobile_item(
                                    "item-docx",
                                    "📘 Export DOCX",
                                    cx,
                                    |this, window, cx| {
                                        this.show_mobile_menu = false;
                                        this.handle_export_docx(
                                            &actions::FileExportDocx,
                                            window,
                                            cx,
                                        );
                                    },
                                ))
                                .child(self.render_mobile_item(
                                    "item-pkg",
                                    "📦 Package Manager",
                                    cx,
                                    |this, window, cx| {
                                        this.show_mobile_menu = false;
                                        this.handle_package_manager(
                                            &actions::PackageManager,
                                            window,
                                            cx,
                                        );
                                    },
                                )),
                        ),
                ),
        )
    }

    fn render_mobile_item(
        &self,
        id: &'static str,
        label: &'static str,
        cx: &mut Context<Self>,
        handler: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .id(id)
            .h(px(34.0))
            .px_3()
            .rounded_md()
            .flex()
            .items_center()
            .cursor_pointer()
            .bg(theme.secondary)
            .text_color(theme.foreground)
            .border_1()
            .border_color(theme.border)
            .hover(|style| style.bg(theme.accent))
            .text_xs()
            .on_click(cx.listener(move |this, _, window, cx| handler(this, window, cx)))
            .child(label)
    }
}
