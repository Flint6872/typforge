mod completions;
mod handlers;
mod render;

use crate::editor::{CodeEditor, EditorStateExt, FileContentUpdated, OpenedFile};
use crate::package_manager::{PackageManagerStore, PackageManagerView};
use gpui_kit::App;
use gpui_kit::component::{RopeExt, dock::Panel as DockPanel, input::EditorState};
use gpui_kit::*;
use parking_lot::Mutex;
use std::rc::Rc;
use std::time::Duration;
use std::time::Instant;
use std::{path::PathBuf, sync::Arc};
use typastry::edit::{detect_properties_at_offset, inject_or_update_parameter};
use typastry::format::format_document;
use typastry::inspector::{CursorContext, TypastryInspector, TypastryJump};
use typastry::intel::{Tooltip, TypastrySignatureHelp, get_signature_help};
use typst::syntax::Source;
use typst_gpui::TypstGpuiWorld;

impl EventEmitter<FileContentUpdated> for EditorPanel {}

pub struct EditorSelectionChanged {
    pub is_bold: bool,
    pub is_italic: bool,
    pub size: Option<f32>,
    pub font: Option<String>,
    pub color: Option<String>,
    pub cursor_context: CursorContext,
}

impl EventEmitter<EditorSelectionChanged> for EditorPanel {}

pub struct EditorPanel {
    pub open_files: Vec<OpenedFile>,
    pub active_file_path: Option<PathBuf>,
    focus_handle: FocusHandle,
    pub zoom_level: f32,
    pub world: Box<dyn TypstGpuiWorld>,

    pub current_hover_content: Option<Tooltip>,
    pub current_hover_position: Option<Point<Pixels>>,
    pub last_hover_request_time: Option<Instant>,
    pub current_signature_help: Option<TypastrySignatureHelp>,
    pub package_manager_view: Option<Entity<PackageManagerView>>,
    pub ribbon_task: Option<Task<()>>,
}

impl Clone for EditorPanel {
    fn clone(&self) -> Self {
        Self {
            open_files: self.open_files.clone(),
            active_file_path: self.active_file_path.clone(),
            focus_handle: self.focus_handle.clone(),
            zoom_level: self.zoom_level,
            world: self.world.try_clone().expect("World must be clonable"),
            current_hover_content: self.current_hover_content.clone(),
            current_hover_position: self.current_hover_position.clone(),
            last_hover_request_time: self.last_hover_request_time,
            current_signature_help: self.current_signature_help.clone(),
            package_manager_view: self.package_manager_view.clone(),
            ribbon_task: None,
        }
    }
}

impl EditorPanel {
    pub fn new(
        world_box: Box<dyn TypstGpuiWorld>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            open_files: Vec::new(),
            active_file_path: None,
            focus_handle: cx.focus_handle(),
            zoom_level: 1.0,
            world: world_box,
            current_hover_content: None,
            current_hover_position: None,
            last_hover_request_time: None,
            current_signature_help: None,
            package_manager_view: None,
            ribbon_task: None,
        }
    }

    pub fn move_tab(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        let file = self.open_files.remove(from);
        self.open_files.insert(to, file);
        cx.notify();
    }

    fn request_hover(
        &mut self,
        byte_offset: usize,
        mouse_pos: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let world = self.world.as_ref();
        if let Ok(source) = world.source(world.main()) {
            if let Some(tooltip) = TypastryInspector::tooltip(
                world.as_ide(),
                world.document().as_deref(),
                &source,
                byte_offset,
            ) {
                self.current_hover_content = Some(tooltip);
                self.current_hover_position = Some(mouse_pos);
                cx.notify();
                return;
            }
        }
        self.clear_hover(cx);
    }

    // 5. Update update_ribbon_selection to inspect cursor & signature help:
    // AFTER:
    pub fn update_ribbon_selection(
        &mut self,
        content: &str,
        cursor: usize,
        cx: &mut Context<Self>,
    ) {
        let content = content.to_string();
        let handle = cx.weak_entity();

        // Debounce ribbon updates to 50ms so we don't re-parse AST on every micro-keystroke
        self.ribbon_task = None;
        self.ribbon_task = Some(cx.spawn(move |_view, spawned_async_cx: &mut AsyncApp| {
            let mut async_cx = spawned_async_cx.clone();
            async move {
                async_cx
                    .background_executor()
                    .timer(Duration::from_millis(50))
                    .await;

                let (props, cursor_context, sig_help) = async_cx
                    .background_executor()
                    .spawn(async move {
                        let source = Source::detached(&content);
                        let props = detect_properties_at_offset(&content, cursor);
                        let cursor_context = TypastryInspector::inspect_cursor(&source, cursor);

                        // We'll calculate signature help here too
                        // (Assuming update_signature_help can be refactored to return the data)
                        (props, cursor_context, None::<()>)
                    })
                    .await;

                let _ = handle.update(&mut async_cx, |this, cx| {
                    // Update active signature help (refactor this call if it needs the source)
                    // this.update_signature_help(&source, cursor, cx);

                    cx.emit(EditorSelectionChanged {
                        is_bold: props.is_bold,
                        is_italic: props.is_italic,
                        size: props.size,
                        font: props.font,
                        color: props.color,
                        cursor_context,
                    });
                    cx.notify();
                });
            }
        }));
    }

    pub fn update_signature_help(
        &mut self,
        source: &Source,
        cursor: usize,
        cx: &mut Context<Self>,
    ) {
        let world = self.world.as_ref();
        let help = get_signature_help(world.as_ide(), None, source, cursor); // Use .as_ide()

        if self.current_signature_help != help {
            self.current_signature_help = help;
            cx.notify();
        }
    }

    // 6. AST Parameter Injection method for Ribbons / Toolbars:
    pub fn inject_parameter_at_cursor(
        &mut self,
        key: &str,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(active_path) = &self.active_file_path {
            if let Some(file) = self.open_files.iter_mut().find(|f| f.path == *active_path) {
                let content = file.editor_state.read(cx).text().to_string();
                let cursor = file.editor_state.read(cx).cursor();

                if let Some(edit) = inject_or_update_parameter(&content, cursor, key, value) {
                    file.editor_state.update(cx, |state, input_cx| {
                        state.replace_range_with_history(
                            edit.range,
                            &edit.new_text,
                            window,
                            input_cx,
                        );
                        state.set_selected_range(edit.new_selection, input_cx);
                    });
                }
            }
        }
    }

    // 7. Jump from Click (canvas preview to editor source):
    pub fn handle_canvas_jump(
        &mut self,
        page: usize,
        x_pt: f64,
        y_pt: f64,
        _window: &mut Window, // Changed to _window as we'll use the Context's App
        cx: &mut Context<Self>,
    ) {
        let jump = {
            let world_lock = self.world.as_ref();
            let document = world_lock.document();
            if let Some(doc) = document.as_deref() {
                TypastryInspector::jump_from_click(&*world_lock, doc, page, x_pt, y_pt)
            } else {
                None
            }
        };

        if let Some(jump) = jump {
            match jump {
                TypastryJump::File { vpath, offset } => {
                    let path = PathBuf::from(vpath);
                    if let Ok(()) = self.open_file(path.clone(), _window, cx) {
                        // Pass _window from arguments
                        if let Some(offset) = offset {
                            if let Some(file) = self.open_files.iter_mut().find(|f| f.path == path)
                            {
                                file.editor_state.update(cx, |state, input_cx| {
                                    state.set_selected_range(offset..offset, input_cx);
                                });
                            }
                        }
                    }
                }
                TypastryJump::Url(url) => {
                    cx.open_url(&url);
                }
                TypastryJump::Position { .. } => {}
            }
        }
    }

    pub fn clear_hover(&mut self, cx: &mut Context<Self>) {
        self.current_hover_content = None;
        self.current_hover_position = None;
        cx.notify();
    }

    pub fn undo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(active_path) = &self.active_file_path {
            if let Some(file) = self.open_files.iter_mut().find(|f| f.path == *active_path) {
                file.editor_state.update(cx, |state, cx| {
                    state.focus(window, cx);
                });
                window.dispatch_action(Box::new(gpui_kit::component::input::Undo), cx);
            }
        }
    }

    pub fn redo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(active_path) = &self.active_file_path {
            if let Some(file) = self.open_files.iter_mut().find(|f| f.path == *active_path) {
                file.editor_state.update(cx, |state, cx| {
                    state.focus(window, cx);
                });
                window.dispatch_action(Box::new(gpui_kit::component::input::Redo), cx);
            }
        }
    }

    fn subscribe_to_editor_changes(
        _this: &mut Self,
        path: PathBuf,
        editor_state: &Entity<EditorState>,
        cx: &mut Context<Self>,
    ) {
        // 1. Subscribe to actual Change events to sync the workspace world and handle jumping
        cx.subscribe(
            editor_state,
            move |this_view, editor_entity, event: &gpui_kit::component::input::InputEvent, cx| {
                if let gpui_kit::component::input::InputEvent::Change = event {
                    if let Some(_file) = this_view.open_files.iter_mut().find(|f| f.path == path) {
                        let content = editor_entity.read(cx).text().to_string();
                        let cursor = editor_entity.read(cx).cursor();

                        // CURSOR AUTO-JUMP HOOK (Cheap local string check)
                        if cursor >= 2 {
                            if let Some(slice) = content.get(cursor - 2..cursor) {
                                if slice == "()" {
                                    editor_entity.update(cx, |state, input_cx| {
                                        state.set_selected_range(
                                            (cursor - 1)..(cursor - 1),
                                            input_cx,
                                        );
                                    });
                                }
                            }
                        }

                        // Emit update event so preview compiles (CRITICAL)
                        // Note: Redundant main-thread world syncing removed.
                        // PreviewPanel now handles world syncing in its background task.
                        cx.emit(FileContentUpdated {
                            path: Some(path.clone()),
                            content,
                        });
                        cx.notify();
                    }
                }
            },
        )
        .detach();

        // 2. Observe the state of the editor to capture cursor movements and selection changes in real-time
        cx.observe(editor_state, move |this_view, editor_entity, cx| {
            let content = editor_entity.read(cx).text().to_string();
            let cursor = editor_entity.read(cx).cursor();
            this_view.update_ribbon_selection(&content, cursor, cx);
        })
        .detach();
    }

    pub fn open_package_manager(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pkg_path = PathBuf::from("typforge://package-manager");

        // If already open in open_files, just switch to it
        if self.open_files.iter().any(|f| f.path == pkg_path) {
            self.active_file_path = Some(pkg_path);
            cx.notify();
            return;
        }

        // Lazy-init the package manager view on first open
        if self.package_manager_view.is_none() {
            let store = Arc::new(std::sync::Mutex::new(PackageManagerStore::new()));
            let view = cx.new(|cx| PackageManagerView::new(store, window, cx));
            self.package_manager_view = Some(view);
        }

        let editor_state = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("text")
                .default_value("")
        });
        let code_editor_entity =
            cx.new(|_cx| CodeEditor::new(editor_state.clone(), "text".to_string(), Vec::new()));

        let tab_file = OpenedFile {
            path: pkg_path.clone(),
            editor_state,
            language: "pkg".to_string(),
            has_unsaved_changes: false,
            lsp_version: 0,
            diagnostics: Vec::new(),
            code_editor_entity,
            is_untitled: false,
        };

        self.open_files.push(tab_file);
        self.active_file_path = Some(pkg_path);
        cx.notify();
    }
}

impl EditorPanel {
    pub fn set_zoom(&mut self, zoom: f32, cx: &mut gpui::Context<Self>) {
        self.zoom_level = zoom.clamp(0.25, 5.0);
        cx.notify();
    }

    pub fn open_file(
        &mut self,
        mut path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        path = path.canonicalize().unwrap_or(path);

        if let Some(existing_file_index) = self.open_files.iter().position(|f| f.path == path) {
            let existing_file = &mut self.open_files[existing_file_index];
            let editor_state_handle = existing_file.editor_state.clone();
            existing_file.lsp_version += 1;

            let content = editor_state_handle.read(cx).text().to_string();

            self.active_file_path = Some(path.clone());
            editor_state_handle.update(cx, |state, cx| {
                state.focus(window, cx);
            });

            self.world.set_source(content.clone());

            cx.emit(FileContentUpdated {
                path: Some(path),
                content,
            });
            cx.notify();
            return Ok(());
        }

        match OpenedFile::new(path.clone(), window, cx) {
            Ok(mut new_file) => {
                let provider =
                    completions::TypstCompletionProvider::new(self.world.try_clone().unwrap());

                new_file.editor_state.update(cx, |state, _| {
                    state.lsp_mut().completion_provider = Some(Rc::new(provider));
                });

                let content = new_file.editor_state.read(cx).text().to_string();
                new_file.lsp_version += 1;

                self.open_files.push(new_file.clone());
                self.active_file_path = Some(path.clone());

                EditorPanel::subscribe_to_editor_changes(
                    self,
                    path.clone(),
                    &new_file.editor_state,
                    cx,
                );

                self.world.set_source(content.clone());

                cx.emit(FileContentUpdated {
                    path: Some(path),
                    content,
                });
                cx.notify();
                Ok(())
            }
            Err(e) => {
                eprintln!("EditorPanel: Failed to open file {:?}: {}", path, e);
                Err(e)
            }
        }
    }

    pub fn close_file(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if let Some(pos) = self.open_files.iter().position(|f| f.path == path) {
            self.open_files.remove(pos);

            if self.active_file_path == Some(path.clone()) {
                if self.open_files.is_empty() {
                    self.active_file_path = None;
                    cx.emit(FileContentUpdated {
                        path: Some(path),
                        content: String::new(),
                    });
                } else {
                    let new_pos = pos.min(self.open_files.len() - 1);
                    let new_active_path = self.open_files[new_pos].path.clone();
                    self.active_file_path = Some(new_active_path.clone());

                    if let Some(active_file) =
                        self.open_files.iter().find(|f| f.path == new_active_path)
                    {
                        let content = active_file.editor_state.read(cx).text().to_string();
                        self.world.set_source(content.clone());

                        cx.emit(FileContentUpdated {
                            path: Some(new_active_path),
                            content,
                        });
                    }
                }
            }
            cx.notify();
        }
    }

    pub fn save_active_file(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(active_path) = &self.active_file_path {
            if let Some(file) = self.open_files.iter_mut().find(|f| f.path == *active_path) {
                let original_content = file.editor_state.read(cx).text().to_string();
                let mut content_to_save = original_content.clone();

                // Only format .typ files
                if active_path.extension().map_or(false, |ext| ext == "typ") {
                    // You might want to make line_width configurable
                    match format_document(&original_content, 80) {
                        Ok(formatted_text) => {
                            content_to_save = formatted_text;
                            // Update the editor's state with the formatted content
                            file.editor_state.update(cx, |state, input_cx| {
                                // 1. Save current cursor byte offset and selection range
                                let old_cursor_byte_offset = state.cursor();
                                let old_selection_range = state.selected_range();

                                // 2. Replace the document content while preserving undo history
                                let current_len = state.text().len();
                                state.replace_range_with_history(
                                    0..current_len,
                                    &content_to_save,
                                    _window,
                                    input_cx,
                                );

                                // 3. Attempt to restore the cursor position and selection
                                //    We clamp the old byte offset to the new document length.
                                let new_doc_len = state.text().len();
                                let new_cursor_byte_offset =
                                    old_cursor_byte_offset.min(new_doc_len);
                                let new_selection_start =
                                    old_selection_range.start.min(new_doc_len);
                                let new_selection_end = old_selection_range.end.min(new_doc_len);

                                // If there was a selection, restore it; otherwise, just restore the cursor.
                                if !old_selection_range.is_empty()
                                    && new_selection_start != new_selection_end
                                {
                                    state.set_selected_range(
                                        new_selection_start..new_selection_end,
                                        input_cx,
                                    );
                                } else {
                                    // Convert byte offset to Position and then set the cursor.
                                    let new_cursor_pos =
                                        state.text().offset_to_position(new_cursor_byte_offset);
                                    state.set_cursor_position(new_cursor_pos, _window, input_cx);
                                }
                            });
                        }
                        Err(e) => {
                            eprintln!("Failed to format file {}: {}", active_path.display(), e);
                            // Continue saving original content if formatting fails
                        }
                    }
                }

                if let Err(e) = std::fs::write(active_path, &content_to_save) {
                    eprintln!("Failed to save file: {}", e);
                    return;
                }

                file.has_unsaved_changes = false;
                file.lsp_version += 1;
                file.is_untitled = false;

                cx.emit(FileContentUpdated {
                    path: Some(active_path.clone()),
                    content: content_to_save.to_string(),
                });
            }
        }
    }

    pub fn new_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        eprintln!("--- NEW FILE CALLED ---");
        let mut i = 1;
        let mut candidate_filename: PathBuf;
        let temp_dir = std::env::temp_dir();

        loop {
            candidate_filename = temp_dir.join(format!("untitled_{}.typ", i));
            if !self.open_files.iter().any(|f| f.path == candidate_filename) {
                break;
            }
            i += 1;
        }

        if let Some(parent) = candidate_filename.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let editor_state = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("typst")
                .default_value("")
        });

        let provider = completions::TypstCompletionProvider::new(self.world.try_clone().unwrap());

        editor_state.update(cx, |state, _| {
            state.lsp_mut().completion_provider = Some(Rc::new(provider));
        });

        let code_editor_entity =
            cx.new(|_cx| CodeEditor::new(editor_state.clone(), "typst".to_string(), Vec::new()));

        let mut new_file = OpenedFile {
            path: candidate_filename.clone(),
            editor_state: editor_state.clone(),
            language: "typst".to_string(),
            has_unsaved_changes: true,
            lsp_version: 0,
            diagnostics: Vec::new(),
            code_editor_entity,
            is_untitled: true,
        };
        new_file.lsp_version += 1;

        self.open_files.push(new_file.clone());
        self.active_file_path = Some(candidate_filename.clone());

        EditorPanel::subscribe_to_editor_changes(
            self,
            candidate_filename.clone(),
            &new_file.editor_state,
            cx,
        );

        cx.emit(FileContentUpdated {
            path: Some(candidate_filename),
            content: String::new(),
        });
        cx.notify();
    }

    pub fn save_file_as(
        &mut self,
        new_path: PathBuf,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        if let Some(active_path_mut) = &mut self.active_file_path {
            if let Some(file_index) = self
                .open_files
                .iter()
                .position(|f| f.path == *active_path_mut)
            {
                let file = &mut self.open_files[file_index];
                let content = file.editor_state.read(cx).text();

                std::fs::write(&new_path, &content.to_string())?;

                file.path = new_path.clone();
                *active_path_mut = new_path.clone();
                file.has_unsaved_changes = false;
                file.lsp_version += 1;
                file.is_untitled = false;

                cx.emit(FileContentUpdated {
                    path: Some(new_path),
                    content: content.to_string(),
                });
                cx.notify();
                return Ok(());
            }
        }
        Err(std::io::Error::new(std::io::ErrorKind::Other, "No active file to save as.").into())
    }
}

impl Focusable for EditorPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<gpui_kit::component::dock::PanelEvent> for EditorPanel {}

impl gpui_kit::base::dock::Panel for EditorPanel {
    fn panel_name(&self) -> &'static str {
        "EditorPanel"
    }
}

impl DockPanel for EditorPanel {
    fn title(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child("Editor")
    }
}
