//typforge0.0.1\crates\typforge\src\workspace\mod.rs
pub mod handlers;
pub mod render;

use gpui::*;
use std::sync::Arc;

// Import necessary types
use crate::{
    actions::{self, RibbonAction},
    editor::{
        FileContentUpdated, editor_panel::EditorPanel, editor_panel::EditorSelectionRangeChanged,
    },
    panels::{FilesPanel, OpenFileEvent},
    ribbon::panel::RibbonPanel,
};
use gpui_component::{
    RopeExt,
    dock::{DockArea, DockItem},
    menu::AppMenuBar,
};
use gpui_util::ResultExt;
use parking_lot::Mutex;

use typforge_core::edit::apply_edit_action;
use typst_gpui::{PreviewPanel, PreviewPanelEvent};

pub struct TypstNoteView<W: typst_gpui::TypstGpuiWorld + typforge_core::IdeWorld> {
    pub dock_area: Entity<DockArea>,
    pub menu_bar: Option<Entity<AppMenuBar>>,
    pub ribbon_panel: Entity<RibbonPanel>,
    pub editor_panel: Entity<EditorPanel<W>>,
    pub preview_panel: Entity<PreviewPanel<W>>,
    pub files_panel: Entity<FilesPanel>,
    pub window_handle: AnyWindowHandle,
}

impl<W: typst_gpui::TypstGpuiWorld + typforge_core::IdeWorld> TypstNoteView<W> {
    pub fn new(
        window: &mut Window,
        shared_world_arc: Arc<Mutex<W>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let files_panel = cx.new(|cx| FilesPanel::new(window, cx));
        let editor_panel_entity =
            cx.new(|cx| EditorPanel::new(shared_world_arc.clone(), window, cx));

        editor_panel_entity.update(cx, |editor_panel, cx| {
            editor_panel.new_file(window, cx);
        });

        let font_families: Vec<String> = {
            let world = shared_world_arc.lock();
            world
                .book()
                .families()
                .map(|(name, _)| name.to_string())
                .collect()
        };

        let preview_panel = cx.new(|cx| PreviewPanel::new(shared_world_arc, window, cx));
        let ribbon_panel = cx.new(|cx| RibbonPanel::new(font_families, window, cx));

        let window_handle = window.window_handle();
        let editor_panel_entity_clone_for_subscriptions = editor_panel_entity.clone();
        let preview_panel_clone_for_subscriptions = preview_panel.clone();
        let files_panel_clone_for_subscriptions = files_panel.clone();

        cx.subscribe(
            &ribbon_panel,
            move |this_note_view, _emitter, event, cx_for_note_view| {
                this_note_view.handle_ribbon_action(event, cx_for_note_view);
            },
        )
        .detach();

        // --- 2. Editor -> Preview Synchronization (via FileContentUpdated events) ---
        cx.subscribe(
            &editor_panel_entity,
            move |this_note_view: &mut TypstNoteView<W>,
                  _emitter: Entity<EditorPanel<W>>,
                  event: &FileContentUpdated,
                  cx_for_note_view: &mut Context<TypstNoteView<W>>| {
                let content = event.content.clone();
                let path = event.path.clone();

                let _ = this_note_view
                    .window_handle
                    .update(cx_for_note_view, |_, window_ref, app_cx_from_handle| {
                        this_note_view.preview_panel.update(
                            app_cx_from_handle,
                            |preview, cx_for_preview| {
                                preview.update_document_info(
                                    path,
                                    content.clone(),
                                    window_ref,
                                    cx_for_preview,
                                );
                                preview.set_source(content, window_ref, cx_for_preview);
                            },
                        );
                    })
                    .log_err();
            },
        )
        .detach();

        // --- 2b. Editor -> Preview Selection Synchronization ---
        let preview_panel_for_sel_sync = preview_panel.clone();
        let editor_panel_for_sel_sync = editor_panel_entity.clone();
        cx.subscribe(
            &editor_panel_entity,
            move |this_note_view,
                  _emitter,
                  event: &EditorSelectionRangeChanged,
                  cx_for_note_view| {
                let range = event.range.clone();
                let preview_panel_handle = preview_panel_for_sel_sync.clone();
                let editor_panel_handle = editor_panel_for_sel_sync.clone();
                let window_handle = this_note_view.window_handle.clone();

                let _ = window_handle.update(cx_for_note_view, |_, window, app_cx| {
                    // Bulletproof Check: Is the active file's text editor focused?
                    let editor_focused =
                        editor_panel_handle.read(app_cx).open_files.iter().any(|f| {
                            f.editor_state
                                .read(app_cx)
                                .focus_handle(app_cx)
                                .is_focused(window)
                        });

                    if editor_focused {
                        preview_panel_handle.update(app_cx, |preview, cx| {
                            if preview.selection_range() != Some(range.clone()) {
                                preview.set_selection(range, window, cx);
                            }
                        });
                    }
                });
            },
        )
        .detach();

        let initial_editor_state_content = editor_panel_entity
            .read(cx)
            .active_file_path
            .clone()
            .and_then(|path| {
                editor_panel_entity
                    .read(cx)
                    .open_files
                    .iter()
                    .find(|f| f.path == path)
                    .map(|f| f.editor_state.read(cx).text().to_string())
            });

        if let Some(content_from_initial_read) = initial_editor_state_content {
            let active_file_path_from_initial_read =
                editor_panel_entity.read(cx).active_file_path.clone();

            let content_clone = content_from_initial_read.clone();
            let path_clone = active_file_path_from_initial_read.clone();

            editor_panel_entity.update(cx, move |editor_panel_view, cx_for_editor_panel| {});
        } else {
            let _ = window_handle
                .clone()
                .update(cx, |_, window_ref, app_cx_in_init| {
                    preview_panel.update(app_cx_in_init, |panel, cx_for_preview| {
                        panel.set_source("Hello, Typst!".to_string(), window_ref, cx_for_preview);
                        panel.update_document_info(
                            None,
                            "Hello, Typst!".to_string(),
                            window_ref,
                            cx_for_preview,
                        );
                    });
                })
                .log_err();
        }

        // --- 4. Preview -> Editor Synchronization (including selection highlights) ---
        let editor_panel_handle_for_preview_sync =
            editor_panel_entity_clone_for_subscriptions.clone();
        cx.subscribe(
            &preview_panel,
            move |this_note_view, _emitter, event, cx_for_note_view| {
                match event {
                    PreviewPanelEvent::SourceChanged(new_content) => {
                        let content = new_content.clone();
                        let editor_panel_handle = this_note_view.editor_panel.clone();
                        let window_handle = this_note_view.window_handle.clone();

                        let _ = window_handle
                            .update(cx_for_note_view, |_, window, app_cx| {
                                editor_panel_handle.update(app_cx, |editor, editor_cx| {
                                    if let Some(active_path) = &editor.active_file_path {
                                        if let Some(file) = editor
                                            .open_files
                                            .iter_mut()
                                            .find(|f| &f.path == active_path)
                                        {
                                            file.editor_state.update(
                                                editor_cx,
                                                |state, input_cx| {
                                                    state.set_value(content, window, input_cx);
                                                },
                                            );
                                            file.has_unsaved_changes = true;
                                        }
                                    }
                                });
                            })
                            .log_err();
                    }
                    PreviewPanelEvent::DiagnosticsChanged(diags) => {
                        this_note_view
                            .editor_panel
                            .update(cx_for_note_view, |panel, cx| {
                                if let Some(active_path) = &panel.active_file_path {
                                    if let Some(file) =
                                        panel.open_files.iter_mut().find(|f| f.path == *active_path)
                                    {
                                        file.diagnostics = diags.clone();

                                        let world = panel.shared_world.lock();
                                        let main_id = world.main();
                                        if let Ok(source) = world.source(main_id) {
                                            file.code_editor_entity.update(cx, |editor, cx| {
                                                editor.set_diagnostics(diags.clone(), &source, cx);
                                            });
                                        }
                                    }
                                }
                                cx.notify();
                            });
                    }
                    PreviewPanelEvent::SelectionChanged(range) => {
                        // --- Push Selection Highlight back to Code Editor (Direct Focus Check) ---
                        let range = range.clone();
                        let editor_panel_handle = this_note_view.editor_panel.clone();
                        let preview_panel_handle = this_note_view.preview_panel.clone();
                        let window_handle = this_note_view.window_handle.clone();

                        let _ = window_handle
                            .update(cx_for_note_view, |_, window, app_cx| {
                                // Bulletproof Check: Is the preview pane's text buffer focused?
                                let preview_focused = preview_panel_handle
                                    .read(app_cx)
                                    .input_state
                                    .read(app_cx)
                                    .focus_handle(app_cx)
                                    .is_focused(window);

                                if preview_focused {
                                    editor_panel_handle.update(app_cx, |editor, editor_cx| {
                                        if let Some(active_path) = &editor.active_file_path {
                                            if let Some(file) = editor
                                                .open_files
                                                .iter_mut()
                                                .find(|f| f.path == *active_path)
                                            {
                                                file.editor_state.update(
                                                    editor_cx,
                                                    |state, input_cx| {
                                                        if state.selected_range() != range {
                                                            state.set_selected_range(
                                                                range.clone(),
                                                                input_cx,
                                                            );
                                                            let new_pos = state
                                                                .text()
                                                                .offset_to_position(range.end);
                                                            state.set_cursor_position(
                                                                new_pos, window, input_cx,
                                                            );
                                                        }
                                                    },
                                                );
                                            }
                                        }
                                    });
                                }
                            })
                            .log_err();
                    }
                }
            },
        )
        .detach();

        cx.subscribe(
            &files_panel,
            move |_this, _emitter, event: &OpenFileEvent, cx_for_note_view| {
                let path = event.path.clone();
                let editor_panel_handle = editor_panel_entity_clone_for_subscriptions.clone();
                let window_handle = window_handle.clone();

                if path.is_dir() {
                    files_panel_clone_for_subscriptions.update(
                        cx_for_note_view,
                        |panel, panel_cx| {
                            panel.on_item_expanded(path.to_string_lossy().to_string(), panel_cx);
                        },
                    );
                } else {
                    cx_for_note_view
                        .spawn(move |_, spawned_async_cx: &mut AsyncApp| {
                            let mut async_cx = spawned_async_cx.clone();
                            async move {
                                window_handle
                                    .update(&mut async_cx, |_, window_ref, app_cx| {
                                        editor_panel_handle.update(
                                            app_cx,
                                            |editor_panel_view, editor_cx| {
                                                let _ = editor_panel_view
                                                    .open_file(path, window_ref, editor_cx);
                                            },
                                        );
                                    })
                                    .log_err();
                            }
                        })
                        .detach();
                }
            },
        )
        .detach();

        let dock_area_entity = cx.new(|cx| {
            let mut dock_area = DockArea::new("main-dock", Some(1), window, cx);
            let weak_dock_area = cx.entity().downgrade();

            let dock_items = vec![
                DockItem::tabs(
                    vec![std::sync::Arc::new(files_panel.clone())],
                    &weak_dock_area,
                    window,
                    cx,
                )
                .size(px(150.0)),
                DockItem::tabs(
                    vec![std::sync::Arc::new(editor_panel_entity.clone())],
                    &weak_dock_area,
                    window,
                    cx,
                ),
                DockItem::tabs(
                    vec![std::sync::Arc::new(preview_panel.clone())],
                    &weak_dock_area,
                    window,
                    cx,
                ),
            ];

            dock_area.set_center(
                DockItem::h_split(dock_items, &weak_dock_area, window, cx),
                window,
                cx,
            );
            dock_area
        });

        #[cfg(not(target_os = "macos"))]
        let menu_bar = Some(gpui_component::menu::AppMenuBar::new(cx));
        #[cfg(target_os = "macos")]
        let menu_bar = None;

        Self {
            dock_area: dock_area_entity,
            menu_bar,
            ribbon_panel,
            editor_panel: editor_panel_entity,
            preview_panel,
            files_panel,
            window_handle,
        }
    }

    pub fn handle_ribbon_action(&mut self, action: &RibbonAction, cx: &mut Context<Self>) {
        let editor_panel = self.editor_panel.clone();
        let preview_panel = self.preview_panel.clone();
        let window_handle = self.window_handle.clone();

        let _ = window_handle
            .update(cx, |_, window, app_cx| {
                let is_preview_focused = preview_panel
                    .read(app_cx)
                    .focus_handle(app_cx)
                    .contains_focused(window, app_cx);

                let preview_selection = if is_preview_focused {
                    preview_panel.read(app_cx).selection_range()
                } else {
                    None
                };

                editor_panel.update(app_cx, |editor, editor_cx| {
                    if let Some(active_path) = &editor.active_file_path {
                        if let Some(file) = editor
                            .open_files
                            .iter_mut()
                            .find(|f| &f.path == active_path)
                        {
                            let mut final_new_selection = None;

                            file.editor_state.update(editor_cx, |state, input_cx| {
                                let content = state.text().to_string();
                                let selection = preview_selection
                                    .clone()
                                    .unwrap_or_else(|| state.selected_range());

                                let edit = apply_edit_action(&content, selection, &action.into());

                                state.replace_range_with_history(
                                    edit.range,
                                    &edit.new_text,
                                    window,
                                    input_cx,
                                );

                                state.set_selected_range(edit.new_selection.clone(), input_cx);
                                final_new_selection = Some(edit.new_selection);

                                if !is_preview_focused {
                                    state.focus(window, input_cx);
                                }
                            });

                            file.has_unsaved_changes = true;

                            if is_preview_focused {
                                if let Some(ref new_sel) = final_new_selection {
                                    preview_panel.update(editor_cx, |preview, preview_cx| {
                                        preview.set_selection(new_sel.clone(), window, preview_cx);
                                    });
                                }
                            }

                            // 3. Immediately compile and push updated content to Preview
                            let content = file.editor_state.read(editor_cx).text().to_string();
                            editor_cx.emit(FileContentUpdated {
                                path: Some(active_path.clone()),
                                content,
                            });
                        }
                    }
                });
            })
            .log_err();
    }
}
