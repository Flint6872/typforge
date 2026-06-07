mod handlers;
mod render;

use crate::editor::{CodeEditor, FileContentUpdated, OpenedFile};
use crate::typst_world::GpuiWorld;
use gpui::*;
use gpui_component::{dock::Panel as DockPanel, input::InputState};
use parking_lot::{Mutex, RawMutex};
use std::time::Instant;
use std::{path::PathBuf, sync::Arc};
use typst_gpui::TypstGpuiWorld;

use typforge_core::intel::{Completion, Tooltip, get_completions, get_hover_info};

impl<W: typst::World + typforge_core::IdeWorld + 'static> EventEmitter<FileContentUpdated>
    for EditorPanel<W>
{
}

pub struct EditorPanel<W: typst::World + typforge_core::IdeWorld + 'static> {
    pub open_files: Vec<OpenedFile>,
    pub active_file_path: Option<PathBuf>,
    focus_handle: FocusHandle,
    pub zoom_level: f32,
    pub shared_world: Arc<Mutex<W>>,

    pub current_hover_content: Option<Tooltip>,
    pub current_hover_position: Option<Point<Pixels>>,
    pub last_hover_request_time: Option<Instant>,
    pub completions: Vec<Completion>,
    pub completions_trigger_index: Option<usize>,
}

impl<W: typst::World + typforge_core::IdeWorld + typst_gpui::TypstGpuiWorld + 'static> Clone
    for EditorPanel<W>
{
    fn clone(&self) -> Self {
        Self {
            open_files: self.open_files.clone(),
            active_file_path: self.active_file_path.clone(),
            focus_handle: self.focus_handle.clone(),
            zoom_level: self.zoom_level,
            shared_world: self.shared_world.clone(),
            current_hover_content: self.current_hover_content.clone(),
            current_hover_position: self.current_hover_position.clone(),
            last_hover_request_time: self.last_hover_request_time,
            completions: Vec::new(),
            completions_trigger_index: None,
        }
    }
}

impl<W: typst::World + typforge_core::IdeWorld + typst_gpui::TypstGpuiWorld + 'static>
    EditorPanel<W>
{
    pub fn new(shared_world: Arc<Mutex<W>>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            open_files: Vec::new(),
            active_file_path: None,
            focus_handle: cx.focus_handle(),
            zoom_level: 1.0,
            shared_world,
            current_hover_content: None,
            current_hover_position: None,
            last_hover_request_time: None,
            completions: Vec::new(),
            completions_trigger_index: None,
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
        // 1. Get the data we need in a scope
        let tooltip = {
            let world_lock = self.shared_world.lock();
            let main_id = world_lock.main();

            if let Ok(source) = world_lock.source(main_id) {
                // Return the tooltip, dropping the lock immediately after this block
                get_hover_info(&*world_lock, None, &source, byte_offset)
            } else {
                None
            }
        }; // <--- world_lock is dropped here!

        // 2. Now self is free to be borrowed mutably
        if let Some(tooltip) = tooltip {
            self.current_hover_content = Some(tooltip);
            self.current_hover_position = Some(mouse_pos);
            cx.notify();
        } else {
            self.clear_hover(cx);
        }
    }

    pub fn clear_hover(&mut self, cx: &mut Context<Self>) {
        self.current_hover_content = None;
        self.current_hover_position = None;
        cx.notify();
    }

    fn subscribe_to_editor_changes(
        this: &mut Self,
        path: PathBuf,
        editor_state: &Entity<InputState>,
        cx: &mut Context<Self>,
    ) {
        cx.subscribe(
            editor_state,
            move |this_view, editor_entity, event: &gpui_component::input::InputEvent, cx| {
                if let gpui_component::input::InputEvent::Change = event {
                    if let Some(file) = this_view.open_files.iter_mut().find(|f| f.path == path) {
                        let content = editor_entity.read(cx).text().to_string();

                        // 1. Sync in-memory world
                        {
                            let mut world = this_view.shared_world.lock();
                            world.set_source(content.clone());
                        }

                        let cursor = editor_entity.read(cx).cursor();

                        // 2. State Machine: Initialize session on typing '#'
                        if cursor > 0 && content.chars().nth(cursor - 1) == Some('#') {
                            this_view.completions_trigger_index = Some(cursor - 1);
                        }

                        // 3. State Machine: Keep-Alive & Filter if session is active
                        if let Some(start_idx) = this_view.completions_trigger_index {
                            // Close session if the cursor moves before the '#' or if we type space/delimiters
                            let mut should_close_session = cursor <= start_idx;
                            if !should_close_session && cursor > start_idx {
                                if let Some(last_char) = content.chars().nth(cursor - 1) {
                                    if last_char.is_whitespace()
                                        || last_char == ']'
                                        || last_char == ')'
                                        || last_char == '}'
                                    {
                                        should_close_session = true;
                                    }
                                }
                            }

                            if should_close_session {
                                this_view.completions.clear();
                                this_view.completions_trigger_index = None;
                            } else {
                                // Session is active! Fetch filtered completions continuously
                                let world = this_view.shared_world.lock();
                                let source_result = world.source(world.main());

                                if let Ok(source) = source_result {
                                    let completions =
                                        get_completions(&*world, None, &source, cursor, false);
                                    this_view.completions = completions;
                                }
                            }
                        }

                        // 4. Emit update event so preview compiles (CRITICAL)
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
    }
}

impl<W: typst::World + typforge_core::IdeWorld + typst_gpui::TypstGpuiWorld + 'static>
    EditorPanel<W>
{
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

            // Set main source content in our compiler world
            {
                let mut world = self.shared_world.lock();
                world.set_source(content.clone());
            }

            cx.emit(FileContentUpdated {
                path: Some(path),
                content,
            });
            cx.notify();
            return Ok(());
        }

        match OpenedFile::new(path.clone(), window, cx) {
            Ok(mut new_file) => {
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

                // Set main source content in our compiler world
                {
                    let mut world = self.shared_world.lock();
                    world.set_source(content.clone());
                }

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
                        {
                            let mut world = self.shared_world.lock();
                            world.set_source(content.clone());
                        }

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
                let content = file.editor_state.read(cx).text();

                if let Err(e) = std::fs::write(active_path, &content.to_string()) {
                    eprintln!("Failed to save file: {}", e);
                    return;
                }

                file.has_unsaved_changes = false;
                file.lsp_version += 1;

                cx.emit(FileContentUpdated {
                    path: Some(active_path.clone()),
                    content: content.to_string(),
                });
            }
        }
    }

    pub fn new_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
            InputState::new(window, cx)
                .code_editor("typst")
                .multi_line(true)
                .soft_wrap(false)
                .line_number(true)
                .searchable(true)
                .folding(true)
                .default_value("")
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

impl<W: typst::World + typforge_core::IdeWorld + 'static> Focusable for EditorPanel<W> {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl<W: typst::World + typforge_core::IdeWorld + 'static>
    EventEmitter<gpui_component::dock::PanelEvent> for EditorPanel<W>
{
}

impl<W: typst::World + typforge_core::IdeWorld + typst_gpui::TypstGpuiWorld + 'static> DockPanel
    for EditorPanel<W>
{
    fn panel_name(&self) -> &'static str {
        "EditorPanel"
    }

    fn title(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child("Editor")
    }
}
