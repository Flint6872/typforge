// crates/typforge/src/workspace/mod.rs

pub mod handlers;
pub mod render;

use gpui_kit::*;
use std::{sync::Arc, time::Duration};

// Import necessary types
use crate::{
    actions::RibbonAction,
    editor::{DiagnosticsPanel, EditorPanel, EditorStateExt, FileContentUpdated},
    panels::{FilesPanel, OpenFileEvent},
    ribbon::RibbonPanel,
};
use typforge_core::CompileOutput;

use gpui_kit::component::{
    dock::{DockArea, DockLayout, DockSkin, panel_handle},
    menu::AppMenuBar,
};
use gpui_util::ResultExt;
use parking_lot::Mutex;

use typastry::{Source, edit::apply_edit_action};
use typforge_core::{DocumentSession, EditOrigin};
use typst_gpui::{PreviewPanel, PreviewPanelEvent}; // ADD THIS IMPORT

pub struct TypstNoteView<W: typst_gpui::TypstGpuiWorld + typastry::IdeWorld> {
    pub dock_area: Entity<DockArea>,
    pub menu_bar: Option<Entity<AppMenuBar>>,
    pub ribbon_panel: Entity<RibbonPanel>,
    pub editor_panel: Entity<EditorPanel<W>>,
    pub diagnostics_panel: Entity<DiagnosticsPanel>,
    pub has_diagnostics: bool,
    pub preview_panel: Entity<PreviewPanel<W>>,
    pub files_panel: Entity<FilesPanel>,
    pub window_handle: AnyWindowHandle,
    pub show_recent_files_picker: bool,
    pub recent_picker_selected_index: Option<usize>,
    pub recent_picker_focus_handle: FocusHandle,
    pub active_selection: Option<std::ops::Range<usize>>,
    pub current_screen_class: crate::workspace::render::ScreenClass,
    pub show_mobile_menu: bool,
    ribbon_inspect_task: Option<Task<()>>,
    editor_sync_task: Option<Task<()>>,
    pub session: DocumentSession,
    pub compiler_host: typforge_core::CompilerHost,
    pub is_compiling: bool,
}

impl<W: typst_gpui::TypstGpuiWorld + typastry::IdeWorld> EventEmitter<FileContentUpdated>
    for TypstNoteView<W>
{
}

impl<W: typst_gpui::TypstGpuiWorld + typastry::IdeWorld> TypstNoteView<W> {
    pub fn new(
        window: &mut Window,
        shared_world_arc: Arc<Mutex<W>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let files_panel = cx.new(|cx| FilesPanel::new(window, cx));
        let editor_panel_entity =
            cx.new(|cx| EditorPanel::new(shared_world_arc.clone(), window, cx));
        let diagnostics_panel = cx.new(|cx| DiagnosticsPanel::new(window, cx));

        let compiler_host = shared_world_arc
            .lock()
            .create_compiler_host()
            .expect("World must support compiler host");

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
        let files_panel_clone_for_subscriptions = files_panel.clone();

        cx.subscribe(&ribbon_panel, move |this, _emitter, event, cx| {
            this.handle_ribbon_action(event, cx);
        })
        .detach();

        cx.observe(&editor_panel_entity, |_this, editor_handle, cx| {
            let editor = editor_handle.read(cx);
            let has_file = !editor.open_files.is_empty();
            crate::components::menus::update_menu_file_state(has_file, cx);
        })
        .detach();

        cx.subscribe(
            &editor_panel_entity,
            move |this, _emitter, event: &FileContentUpdated, cx| {
                if this
                    .session
                    .update(
                        event.path.clone(),
                        event.content.clone(),
                        EditOrigin::Editor,
                    )
                    .is_some()
                {
                    this.trigger_compile(cx);
                }
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

        let mut session = DocumentSession::new();
        let (initial_path, initial_text) = if let Some(content) = initial_editor_state_content {
            let path = editor_panel_entity.read(cx).active_file_path.clone();
            (path, content)
        } else {
            (None, "Hello, Typst!".to_string())
        };

        session.update(initial_path, initial_text, EditOrigin::Disk);

        cx.subscribe(
            &preview_panel,
            move |this, _emitter, event, cx| match event {
                PreviewPanelEvent::SourceChanged(new_content) => {
                    let active_path = this.editor_panel.read(cx).active_file_path.clone();

                    if let Some(target_rev) =
                        this.session
                            .update(active_path, new_content.clone(), EditOrigin::Preview)
                    {
                        this.trigger_compile(cx);
                        this.schedule_editor_sync(target_rev, new_content.clone(), cx);
                    }
                }
                PreviewPanelEvent::UndoRequested => {
                    this.window_handle
                        .update(cx, |_, window, app_cx| {
                            this.editor_panel
                                .update(app_cx, |editor, cx| editor.undo(window, cx));
                        })
                        .log_err();
                }
                PreviewPanelEvent::RedoRequested => {
                    this.window_handle
                        .update(cx, |_, window, app_cx| {
                            this.editor_panel
                                .update(app_cx, |editor, cx| editor.redo(window, cx));
                        })
                        .log_err();
                }
                _ => {}
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
                            let path_for_recent = path.clone();
                            async move {
                                window_handle
                                    .update(&mut async_cx, |_, window_ref, app_cx| {
                                        crate::settings::update_recent_files(
                                            path_for_recent.to_string_lossy().to_string(),
                                            app_cx,
                                        );

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

        let (dock_area_entity, _skin) = DockSkin::dock_area("main-dock", Some(1), window, cx);

        dock_area_entity.update(cx, |dock_area, cx| {
            let layout = DockLayout::h_split()
                .child(
                    DockLayout::tabs().panel_view(panel_handle(files_panel.clone()), cx),
                    Some(px(200.0)),
                )
                .child(
                    DockLayout::v_split()
                        .child(
                            DockLayout::tabs()
                                .panel_view(panel_handle(editor_panel_entity.clone()), cx),
                            None,
                        )
                        .child(
                            DockLayout::tabs()
                                .panel_view(panel_handle(diagnostics_panel.clone()), cx),
                            Some(px(180.0)),
                        ),
                    None,
                )
                .child(
                    DockLayout::tabs().panel_view(panel_handle(preview_panel.clone()), cx),
                    None,
                );

            dock_area.set_center(layout, window, cx);
        });

        #[cfg(not(target_os = "macos"))]
        let menu_bar = Some(gpui_kit::component::menu::AppMenuBar::new(cx));
        #[cfg(target_os = "macos")]
        let menu_bar = None;

        let initial_screen_class =
            crate::workspace::render::ScreenClass::from_width(window.viewport_size().width);

        let mut note_view = Self {
            dock_area: dock_area_entity,
            menu_bar,
            ribbon_panel,
            editor_panel: editor_panel_entity,
            preview_panel,
            diagnostics_panel,
            files_panel,
            window_handle,
            show_recent_files_picker: false,
            recent_picker_selected_index: None,
            recent_picker_focus_handle: cx.focus_handle(),
            active_selection: None,
            has_diagnostics: false,
            current_screen_class: initial_screen_class,
            show_mobile_menu: false,
            ribbon_inspect_task: None,
            editor_sync_task: None,
            session,
            compiler_host,
            is_compiling: false,
        };

        note_view.trigger_compile(cx);
        note_view.update_dock_layout(window, cx);
        note_view
    }

    /// Orchestrates formatting actions triggered from either the Editor or Preview Panel.
    pub fn handle_ribbon_action(&mut self, action: &RibbonAction, cx: &mut Context<Self>) {
        self.editor_sync_task = None;
        let session_text = self.session.text().to_string();

        self.window_handle
            .update(cx, |_, window, app_cx| {
                self.editor_panel.update(app_cx, |editor, editor_cx| {
                    if let Some(active_path) = &editor.active_file_path {
                        if let Some(file) = editor
                            .open_files
                            .iter_mut()
                            .find(|f| f.path == *active_path)
                        {
                            file.editor_state.update(editor_cx, |state, cx| {
                                if state.text().to_string() != session_text {
                                    state.set_value(session_text.clone(), window, cx);
                                }
                                let content = state.text().to_string();
                                let sel = state.selected_range();
                                let edit = apply_edit_action(&content, sel, &action.into());
                                state.replace_range_with_history(
                                    edit.range,
                                    &edit.new_text,
                                    window,
                                    cx,
                                );
                                state.set_selected_range(edit.new_selection, cx);
                            });
                            file.has_unsaved_changes = true;
                            editor_cx.emit(FileContentUpdated {
                                path: Some(active_path.clone()),
                                content: file.editor_state.read(editor_cx).text().to_string(),
                            });
                        }
                    }
                });
            })
            .log_err();
    }

    pub fn update_dock_layout(&mut self, window: &mut Window, cx: &mut App) {
        let screen =
            crate::workspace::render::ScreenClass::from_width(window.viewport_size().width);
        self.current_screen_class = screen;

        let layout = match screen {
            crate::workspace::render::ScreenClass::Compact => {
                // Mobile: All panels share a single tab bar instead of horizontal splits
                let mut tabs = DockLayout::tabs()
                    .panel_view(panel_handle(self.editor_panel.clone()), cx)
                    .panel_view(panel_handle(self.preview_panel.clone()), cx)
                    .panel_view(panel_handle(self.files_panel.clone()), cx);

                if self.has_diagnostics {
                    tabs = tabs.panel_view(panel_handle(self.diagnostics_panel.clone()), cx);
                }

                tabs
            }
            crate::workspace::render::ScreenClass::Medium => {
                // Tablet: Hide file tree or collapse it, split Editor & Preview side-by-side
                let editor_column = if self.has_diagnostics {
                    DockLayout::v_split()
                        .child(
                            DockLayout::tabs()
                                .panel_view(panel_handle(self.editor_panel.clone()), cx),
                            None,
                        )
                        .child(
                            DockLayout::tabs()
                                .panel_view(panel_handle(self.diagnostics_panel.clone()), cx),
                            Some(px(160.0)),
                        )
                } else {
                    DockLayout::tabs().panel_view(panel_handle(self.editor_panel.clone()), cx)
                };

                DockLayout::h_split().child(editor_column, None).child(
                    DockLayout::tabs().panel_view(panel_handle(self.preview_panel.clone()), cx),
                    None,
                )
            }
            crate::workspace::render::ScreenClass::Expanded => {
                // Desktop: Full 3-column layout
                let editor_column = if self.has_diagnostics {
                    DockLayout::v_split()
                        .child(
                            DockLayout::tabs()
                                .panel_view(panel_handle(self.editor_panel.clone()), cx),
                            None,
                        )
                        .child(
                            DockLayout::tabs()
                                .panel_view(panel_handle(self.diagnostics_panel.clone()), cx),
                            Some(px(180.0)),
                        )
                } else {
                    DockLayout::tabs().panel_view(panel_handle(self.editor_panel.clone()), cx)
                };

                DockLayout::h_split()
                    .child(
                        DockLayout::tabs().panel_view(panel_handle(self.files_panel.clone()), cx),
                        Some(px(200.0)),
                    )
                    .child(editor_column, None)
                    .child(
                        DockLayout::tabs().panel_view(panel_handle(self.preview_panel.clone()), cx),
                        None,
                    )
            }
        };

        self.dock_area.update(cx, |dock_area, cx| {
            dock_area.set_center(layout, window, cx);
        });
    }

    fn trigger_compile(&mut self, cx: &mut Context<Self>) {
        let path = self.session.active_path().cloned();
        let text = self.session.text().to_string();

        self.compiler_host.request_compile(path, text);

        if !self.is_compiling {
            self.is_compiling = true;
            self.poll_compiler(cx);
        }
    }

    fn poll_compiler(&mut self, cx: &mut Context<Self>) {
        let rx = self.compiler_host.output_receiver();
        let handle = cx.weak_entity();

        cx.spawn(|_, spawned_cx: &mut AsyncApp| {
            let mut async_cx = spawned_cx.clone();
            async move {
                let output = async_cx
                    .background_executor()
                    .spawn(async move { rx.lock().recv().ok() })
                    .await;

                let _ = handle.update(&mut async_cx, |this, cx| {
                    if let Some(mut latest) = output {
                        // Coalesce: drain anything newer that finished while we were waiting
                        while let Some(newer) = this.compiler_host.try_recv() {
                            latest = newer;
                        }

                        let output_rev = latest.revision; // CAPTURE REVISION HERE
                        this.apply_compilation_result(latest, cx);

                        // Use captured revision to check against current state
                        if output_rev < this.compiler_host.current_revision() {
                            this.poll_compiler(cx);
                        } else {
                            this.is_compiling = false;
                        }
                    }
                });
            }
        })
        .detach();
    }

    fn apply_compilation_result(&mut self, output: CompileOutput, cx: &mut Context<Self>) {
        let diags = output
            .warnings
            .iter()
            .chain(output.errors.iter())
            .cloned()
            .collect::<Vec<_>>();
        let has_errors = diags
            .iter()
            .any(|d| d.severity == typst::diag::Severity::Error);
        let sources = Arc::new(output.sources.clone());

        // 1. Update Preview (Visuals)
        let output_for_preview = output.clone();
        self.preview_panel.update(cx, |preview, cx| {
            preview.apply_compilation_result(output_for_preview, cx);
        });

        // 2. Update Editor Squiggles (Logic)
        self.editor_panel.update(cx, |panel, cx| {
            if let Some(active_path) = &panel.active_file_path {
                if let Some(file) = panel.open_files.iter_mut().find(|f| f.path == *active_path) {
                    file.diagnostics = diags.clone();

                    let source_opt = sources.values().find(|s| {
                        let id = s.id();
                        let vpath = id.vpath().get_without_slash();
                        active_path.to_string_lossy().ends_with(vpath) || vpath == "__main__.typ"
                    });

                    if let Some(src) = source_opt {
                        file.code_editor_entity.update(cx, |ed, cx| {
                            ed.set_diagnostics(diags.clone(), src, cx);
                        });
                    }
                }
            }
        });

        // 3. Update Diagnostics Panel
        self.diagnostics_panel.update(cx, |panel, cx| {
            panel.set_diagnostics(&output.sources, diags, cx);
        });

        if self.has_diagnostics != has_errors {
            self.has_diagnostics = has_errors;
            self.update_dock_layout_cx(cx);
        }
    }

    // Helper to allow updating layout from a Context
    fn update_dock_layout_cx(&mut self, cx: &mut Context<Self>) {
        let window_handle = self.window_handle.clone();
        let _ = window_handle.update(cx, |_this, window, cx| {
            self.update_dock_layout(window, cx);
        });
    }

    fn schedule_editor_sync(&mut self, target_rev: u64, content: String, cx: &mut Context<Self>) {
        let editor_handle = self.editor_panel.clone();
        let window_handle = self.window_handle.clone();
        let note_view_weak = cx.weak_entity();

        self.editor_sync_task = Some(cx.spawn(move |_, spawned_cx: &mut AsyncApp| {
            let mut async_cx = spawned_cx.clone();
            async move {
                async_cx
                    .background_executor()
                    .timer(Duration::from_millis(150))
                    .await;
                let is_latest = note_view_weak
                    .read_with(&async_cx, |this, _| this.session.revision() == target_rev)
                    .unwrap_or(false);
                if !is_latest {
                    return;
                }

                let _ = window_handle.update(&mut async_cx, |_, window, app_cx| {
                    editor_handle.update(app_cx, |editor, cx| {
                        if let Some(path) = &editor.active_file_path {
                            if let Some(file) =
                                editor.open_files.iter_mut().find(|f| &f.path == path)
                            {
                                file.editor_state.update(cx, |state, cx| {
                                    state.set_value(content.clone(), window, cx)
                                });
                                file.has_unsaved_changes = true;
                            }
                        }
                    });
                });
            }
        }));
    }
}
