pub mod handlers;
pub mod render;

use gpui::*;
use std::sync::Arc;

// Import necessary types
use crate::{
    actions::RibbonAction,
    components::lsp::LspClient,
    editor::{FileContentUpdated, editor_panel::EditorPanel},
    panels::{FilesPanel, OpenFileEvent},
    ribbon::panel::RibbonPanel,
};
use gpui_component::{
    dock::{DockArea, DockItem},
    menu::AppMenuBar,
};
use gpui_util::ResultExt;
use parking_lot::Mutex;
use tokio::sync::mpsc;
use typst_gpui::{PreviewPanel, PreviewPanelEvent};
use typstography::PublishDiagnosticsParams;

pub struct TypstNoteView<W: typst_gpui::TypstGpuiWorld> {
    pub dock_area: Entity<DockArea>,
    pub menu_bar: Option<Entity<AppMenuBar>>,
    pub ribbon_panel: Entity<RibbonPanel>,
    pub editor_panel: Entity<EditorPanel>,
    pub preview_panel: Entity<PreviewPanel<W>>,
    pub files_panel: Entity<FilesPanel>,
    pub window_handle: AnyWindowHandle,
    pub lsp_client: Arc<LspClient>,
}

impl<W: typst_gpui::TypstGpuiWorld> TypstNoteView<W> {
    // A constructor for your view.
    pub fn new(
        window: &mut Window,
        shared_world_arc: Arc<Mutex<W>>,
        lsp_client: Arc<LspClient>,
        diagnostics_rx: mpsc::UnboundedReceiver<PublishDiagnosticsParams>,
        responses_rx: mpsc::UnboundedReceiver<serde_json::Value>, // <--- ADDED THIS LINE
        cx: &mut Context<Self>,
    ) -> Self {
        let files_panel = cx.new(|cx| FilesPanel::new(window, cx));
        let editor_panel_entity = cx.new(|cx| {
            EditorPanel::new(lsp_client.clone(), diagnostics_rx, responses_rx, window, cx)
        });

        editor_panel_entity.update(cx, |editor_panel, cx| {
            editor_panel.new_file(window, cx); // Call the new_file method
        });

        let preview_panel = cx.new(|cx| PreviewPanel::new(shared_world_arc, window, cx));
        let ribbon_panel = cx.new(|cx| RibbonPanel::new(cx));

        // --- Handles to be captured by closures ---
        let window_handle = window.window_handle();
        let editor_panel_entity_clone_for_subscriptions = editor_panel_entity.clone();
        let preview_panel_clone_for_subscriptions = preview_panel.clone();
        let files_panel_clone_for_subscriptions = files_panel.clone();

        // --- 1. Ribbon Event Subscription ---
        // This subscription listens to actions coming from our Ribbon UI
        cx.subscribe(
            &ribbon_panel,
            move |this_note_view, _emitter, event, cx_for_note_view| {
                this_note_view.handle_ribbon_action(event, cx_for_note_view);
            },
        )
        .detach();

        // --- 2. Editor -> Preview Synchronization (via FileContentUpdated events) ---
        // This listener ensures that any change originating from the EditorPanel
        // updates the PreviewPanel.
        cx.subscribe(
            &editor_panel_entity,
            move |this_note_view: &mut TypstNoteView<W>,
                  _emitter: Entity<EditorPanel>,
                  event: &FileContentUpdated,
                  cx_for_note_view: &mut Context<TypstNoteView<W>>| {
                // println!(
                //     "Bridge: Received update from editor ({} chars)",
                //     event.content.len()
                // );
                let content = event.content.clone();
                let path = event.path.clone();

                let _ = this_note_view
                    .window_handle
                    .update(cx_for_note_view, |_, window_ref, app_cx_from_handle| {
                        this_note_view.preview_panel.update(
                            app_cx_from_handle,
                            |preview, cx_for_preview| {
                                preview.set_source(content.clone(), window_ref, cx_for_preview);
                                preview.update_document_info(
                                    path.clone(),
                                    content,
                                    window_ref,
                                    cx_for_preview,
                                );
                            },
                        );
                    })
                    .log_err();
            },
        )
        .detach();

        // --- 3. Initial PreviewPanel Content Load ---
        // Trigger an initial update based on editor's current state.
        // This block needs to run once.
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
                editor_panel_entity.read(cx).active_file_path.clone(); // Re-read path

            // The `editor_panel_entity.update` needs to capture these values, so it must be `move`.
            let content_clone = content_from_initial_read.clone();
            let path_clone = active_file_path_from_initial_read.clone();

            editor_panel_entity.update(cx, move |editor_panel_view, cx_for_editor_panel| {});
        } else {
            // If no active file, set a default "Hello, Typst!"
            // Use window_handle to get &mut Window.
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

        // --- 4. Preview -> Editor Synchronization (via PreviewPanelEvent::AppendChar) ---
        // This listener takes input from the PreviewPanel and pushes it to the EditorPanel.
        let editor_panel_handle_for_preview_sync =
            editor_panel_entity_clone_for_subscriptions.clone();
        cx.subscribe(
            &preview_panel,
            move |this_note_view, _emitter, event, cx_for_note_view| {
                if let PreviewPanelEvent::SourceChanged(new_content) = event {
                    let content = new_content.clone();
                    let editor_panel_handle = this_note_view.editor_panel.clone();
                    let window_handle = this_note_view.window_handle.clone();

                    // FIX: Update the window FIRST.
                    // This provides a fresh context (app_cx) and access to &mut Window.
                    let _ = window_handle
                        .update(cx_for_note_view, |_, window, app_cx| {
                            // Use app_cx (provided by the window update) to update the editor
                            editor_panel_handle.update(app_cx, |editor, editor_cx| {
                                if let Some(active_path) = &editor.active_file_path {
                                    if let Some(file) = editor
                                        .open_files
                                        .iter_mut()
                                        .find(|f| &f.path == active_path)
                                    {
                                        // Use editor_cx to update the editor's InputState
                                        file.editor_state.update(editor_cx, |state, input_cx| {
                                            state.set_value(content, window, input_cx);
                                        });
                                        file.has_unsaved_changes = true;

                                        // NOTE: Do NOT emit(FileContentUpdated) here!
                                        // It would trigger an infinite loop back to the Preview.
                                    }
                                }
                            });
                        })
                        .log_err();
                }
            },
        )
        .detach();

        // --- 5. FilesPanel -> EditorPanel (via OpenFileEvent) ---
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

        // --- 6. DockArea and MenuBar Setup ---
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

        // --- 7. Return TypstNoteView instance ---
        Self {
            dock_area: dock_area_entity,
            menu_bar,
            ribbon_panel,
            editor_panel: editor_panel_entity,
            preview_panel,
            files_panel,
            window_handle, // Store the handle
            lsp_client,
        }
    }

    pub fn handle_ribbon_action(&mut self, action: &RibbonAction, _cx: &mut Context<Self>) {
        println!("DEBUG: [Ribbon Event Captured] Action: {:?}", action);
    }
}
