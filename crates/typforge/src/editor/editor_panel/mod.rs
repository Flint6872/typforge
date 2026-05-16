mod handlers;
mod render;

use crate::components::lsp::LspClient;
use crate::editor::{CodeEditor, FileContentUpdated, OpenedFile};
use gpui::*;
use gpui_component::{dock::Panel as DockPanel, input::InputState};
use parking_lot::Mutex;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::{
    sync::{mpsc, oneshot},
    time::Instant,
};
// Use types from typstography (0.94) for the backend communication
use typstography::{Hover, Position as LspPosition, Url};

// Ensure EditorPanel implements EventEmitter for this event
impl EventEmitter<FileContentUpdated> for EditorPanel {}

//#[derive(Clone)]
pub struct EditorPanel {
    pub open_files: Vec<OpenedFile>, // Collection of all open files/tabs
    pub active_file_path: Option<PathBuf>, // Path of the currently focused tab
    focus_handle: FocusHandle,
    pub zoom_level: f32,
    lsp_client: Arc<LspClient>, // Add the LSP client

    // NEW: State for managing LSP responses
    pending_lsp_requests: Arc<Mutex<HashMap<u64, oneshot::Sender<serde_json::Value>>>>,

    // NEW: State for hover display
    current_hover_content: Option<Hover>,
    current_hover_position: Option<Point<Pixels>>, // Screen position for the popup
    last_hover_request_time: Option<Instant>,
    hover_debounce_handle: Option<gpui::Task<Option<Hover>>>,
}

impl Clone for EditorPanel {
    fn clone(&self) -> Self {
        Self {
            open_files: self.open_files.clone(),
            active_file_path: self.active_file_path.clone(),
            focus_handle: self.focus_handle.clone(),
            zoom_level: self.zoom_level,
            lsp_client: self.lsp_client.clone(),
            pending_lsp_requests: self.pending_lsp_requests.clone(),
            current_hover_content: self.current_hover_content.clone(),
            current_hover_position: self.current_hover_position.clone(),
            last_hover_request_time: self.last_hover_request_time.clone(),
            // DO NOT CLONE THE TASK. A new panel should not inherit active debounce tasks.
            hover_debounce_handle: None, // Or Default::default() if the type had one
        }
    }
}

impl EditorPanel {
    pub fn new(
        lsp_client: Arc<LspClient>, // Accept LspClient
        mut diagnostics_rx: mpsc::UnboundedReceiver<tower_lsp::lsp_types::PublishDiagnosticsParams>,
        mut responses_rx: mpsc::UnboundedReceiver<serde_json::Value>, // New receiver
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // NEW: Task for generic responses (Hover, Goto, etc.)
        // This task will now also handle matching responses to pending requests.
        let pending_lsp_requests: Arc<Mutex<HashMap<u64, oneshot::Sender<serde_json::Value>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let pending_clone = pending_lsp_requests.clone();

        // Single Task to handle incoming LSP messages (both diagnostics and responses)
        cx.spawn(|this: WeakEntity<EditorPanel>, spawned_async_cx: &mut AsyncApp| {
            // 1. Clone the reference to get an owned handle
            let mut cx = spawned_async_cx.clone();

            async move {
                loop {
                    tokio::select! {
                        Some(params) = diagnostics_rx.recv() => {
                            // 2. Use the owned 'cx' inside the async block
                            let _ = this.update(&mut cx, |this, cx| {
                                if let Some(file) = this.open_files.iter_mut().find(|f| f.uri() == params.uri) {
                                    file.diagnostics = params.diagnostics.clone();
                                    file.code_editor_entity.update(cx, |editor, cx| {
                                        editor.set_diagnostics(params.diagnostics, cx);
                                    });
                                    cx.notify();
                                }
                            });
                        }
                        Some(msg) = responses_rx.recv() => {
                            if let Some(id) = msg.get("id").and_then(|v| v.as_u64()) {
                                let mut pending = pending_clone.lock();
                                if let Some(tx) = pending.remove(&id) {
                                    let _ = tx.send(msg);
                                }
                            }
                        }
                        else => break,
                    }
                }
            }
        }).detach();

        Self {
            open_files: Vec::new(),
            active_file_path: None,
            focus_handle: cx.focus_handle(),
            zoom_level: 1.0,
            lsp_client, // Store the LSP client
            pending_lsp_requests,
            current_hover_content: None,
            current_hover_position: None,
            last_hover_request_time: None,
            hover_debounce_handle: None,
        }
    }

    /// Moves a tab from one position to another.
    pub fn move_tab(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        let file = self.open_files.remove(from);
        self.open_files.insert(to, file);
        cx.notify();
    }

    fn request_hover(
        &mut self,
        uri: Url,
        position: LspPosition,
        mouse_pos: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let (tx, rx) = oneshot::channel();
        let Some(request_id) = self.lsp_client.hover(uri, position) else {
            return;
        };

        self.pending_lsp_requests.lock().insert(request_id, tx);

        // Use the WeakEntity handle provided by the spawn closure (the first argument)
        self.hover_debounce_handle = Some(cx.spawn(
            move |this: WeakEntity<Self>, spawned_async_cx: &mut AsyncApp| {
                // 1. Create an owned clone of the context
                let mut cx = spawned_async_cx.clone();

                async move {
                    let response = rx.await.ok()?;
                    let result = response.get("result")?.clone();
                    let hover: Hover = serde_json::from_value(result).ok()?;

                    // 2. Use the cloned 'cx' (the owned handle) here
                    this.update(&mut cx, |this: &mut EditorPanel, cx| {
                        this.current_hover_content = Some(hover);
                        this.current_hover_position = Some(mouse_pos);
                        cx.notify();
                    })
                    .ok();

                    None
                }
            },
        ));
    }

    pub fn clear_hover(&mut self, cx: &mut Context<Self>) {
        self.current_hover_content = None;
        self.current_hover_position = None;
        self.hover_debounce_handle = None;
        cx.notify();
    }

    // This is where we need to listen for text changes
    fn subscribe_to_editor_changes(
        this: &mut Self,
        path: PathBuf, // Pass the path to tie the subscription to the file
        editor_state: &Entity<InputState>,
        cx: &mut Context<Self>,
    ) {
        let lsp_client_clone = this.lsp_client.clone();

        // Use gpui_component::input::InputEvent
        cx.subscribe(
            editor_state,
            move |this_view, editor_entity, event: &gpui_component::input::InputEvent, cx| {
                // Match on the Change event
                if let gpui_component::input::InputEvent::Change = event {
                    // Find the specific file that this editor belongs to
                    if let Some(file) = this_view.open_files.iter_mut().find(|f| f.path == path) {
                        file.lsp_version += 1;

                        // Read the current text directly from the editor entity
                        let current_content = editor_entity.read(cx).text().to_string();

                        // Notify LSP
                        lsp_client_clone.did_change(
                            file.uri(),
                            current_content.clone(),
                            file.lsp_version,
                        );

                        // Emit for preview/rendering
                        cx.emit(FileContentUpdated {
                            path: Some(path.clone()),
                            content: current_content,
                        });

                        cx.notify();
                    }
                }
            },
        )
        .detach();
    }
}

impl EditorPanel {
    pub fn set_zoom(&mut self, zoom: f32, cx: &mut gpui::Context<Self>) {
        self.zoom_level = zoom.clamp(0.25, 5.0);
        cx.notify();
    }

    /// Public method to open a file in the editor panel.
    pub fn open_file(
        &mut self,
        mut path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        // --- CRITICAL FIX: Canonicalize the path to ensure it's absolute and resolved ---
        // This makes `Url::from_file_path` more robust.
        path = path.canonicalize().unwrap_or_else(|e| {
            eprintln!("Failed to canonicalize path {:?}: {}", path, e);
            // Fallback: If canonicalize fails, use the original path, but be aware it might still cause URL issues.
            // For a robust app, you might want to return an Err here if canonicalization is critical.
            path.clone()
        });
        // --- END FIX ---

        // 1. Check if file is already open
        if let Some(existing_file_index) = self.open_files.iter().position(|f| f.path == path) {
            let existing_file = &mut self.open_files[existing_file_index];
            let editor_state_handle = existing_file.editor_state.clone();

            // Increment LSP version for an already open document being re-activated
            existing_file.lsp_version += 1;

            let content = editor_state_handle.read(cx).text().to_string();

            self.active_file_path = Some(path.clone());
            editor_state_handle.update(cx, |state, cx| {
                state.focus(window, cx);
            });

            // Re-send initial content to LSP on tab switch to ensure it's up to date
            self.lsp_client.did_change(
                existing_file.uri(),
                content.clone(),
                existing_file.lsp_version,
            );

            cx.emit(FileContentUpdated {
                path: Some(path),
                content,
            });
            cx.notify();
            return Ok(());
        }

        // 2. Open new file (We only reach here if the file wasn't found above)
        match OpenedFile::new(path.clone(), window, cx) {
            Ok(mut new_file) => {
                // Make new_file mutable
                let content = new_file.editor_state.read(cx).text().to_string();

                // Increment version for newly opened file
                new_file.lsp_version += 1;

                self.open_files.push(new_file.clone()); // Clone for later use if needed, or get a ref
                self.active_file_path = Some(path.clone());

                // IMPORTANT: Subscribe to editor changes *after* adding the file
                EditorPanel::subscribe_to_editor_changes(
                    self,
                    path.clone(),
                    &new_file.editor_state,
                    cx,
                );

                // Send initial content to LSP for the newly opened file
                self.lsp_client
                    .did_change(new_file.uri(), content.clone(), new_file.lsp_version);

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

    /// Closes a file and manages the active tab state.
    pub fn close_file(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if let Some(pos) = self.open_files.iter().position(|f| f.path == path) {
            self.open_files.remove(pos);

            // If we closed the active tab, pick a new one
            if self.active_file_path == Some(path.clone()) {
                if self.open_files.is_empty() {
                    self.active_file_path = None;
                    // EMIT: Clear preview if no files left
                    cx.emit(FileContentUpdated {
                        path: Some(path),
                        content: String::new(),
                    });
                } else {
                    let new_pos = pos.min(self.open_files.len() - 1);
                    let new_active_path = self.open_files[new_pos].path.clone();
                    self.active_file_path = Some(new_active_path.clone());

                    // ADD THIS: Emit content update for the new active tab
                    if let Some(active_file) =
                        self.open_files.iter().find(|f| f.path == new_active_path)
                    {
                        let content = active_file.editor_state.read(cx).text().to_string();
                        self.lsp_client.did_change(
                            active_file.uri(),
                            content.clone(),
                            active_file.lsp_version,
                        );

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
                // Make file mutable
                let content = file.editor_state.read(cx).text();

                if let Err(e) = std::fs::write(active_path, &content.to_string()) {
                    eprintln!("Failed to save file: {}", e);
                    return;
                }

                file.has_unsaved_changes = false; // Mark as saved
                file.lsp_version += 1; // Increment LSP version on save (optional, but good practice)

                self.lsp_client
                    .did_change(file.uri(), content.to_string(), file.lsp_version);

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
        let temp_dir = std::env::temp_dir(); // Or a specific app data dir

        loop {
            candidate_filename = temp_dir.join(format!("untitled_{}.typ", i)); // Join with a temp dir
            if !self.open_files.iter().any(|f| f.path == candidate_filename) {
                break;
            }
            i += 1;
        }

        // Ensure the parent directory exists for these temporary files
        if let Some(parent) = candidate_filename.parent() {
            let _ = std::fs::create_dir_all(parent); // Ignore error if it already exists
        }

        let editor_state = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor("typst") // Default to typst for new files
                .multi_line(true)
                .soft_wrap(false)
                .line_number(true)
                .searchable(true)
                .folding(true)
                .default_value("") // Empty content
        });

        let code_editor_entity = cx.new(|_cx| {
            CodeEditor::new(
                editor_state.clone(),
                "typst".to_string(), // Default language for new file
                Vec::new(),          // Initial empty diagnostics
            )
        });

        let mut new_file = OpenedFile {
            path: candidate_filename.clone(), // Use the absolute/temp path
            editor_state: editor_state.clone(),
            language: "typst".to_string(),
            has_unsaved_changes: true,
            lsp_version: 0,
            diagnostics: Vec::new(),
            code_editor_entity,
        };
        new_file.lsp_version += 1;

        self.open_files.push(new_file.clone());
        self.active_file_path = Some(candidate_filename.clone()); // Use the absolute path

        EditorPanel::subscribe_to_editor_changes(
            self,
            candidate_filename.clone(),
            &new_file.editor_state,
            cx,
        );

        self.lsp_client
            .did_change(new_file.uri(), "".to_string(), new_file.lsp_version);

        cx.emit(FileContentUpdated {
            path: Some(candidate_filename), // Ensure this also uses the full path
            content: String::new(),
        });
        cx.notify();
    }

    /// Saves the active file to a specified path, updating its internal path.
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

                // Perform the actual FS save to the new path
                std::fs::write(&new_path, &content.to_string())?;

                // Update the file's path and tab name
                file.path = new_path.clone();
                *active_path_mut = new_path.clone(); // Update the active path in the panel
                file.has_unsaved_changes = false; // Mark as saved
                file.lsp_version += 1; // Increment LSP version on save

                self.lsp_client
                    .did_change(file.uri(), content.to_string(), file.lsp_version);

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

// Implement Focusable for your panel
impl Focusable for EditorPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone() // Return the stable handle
    }
}

// Implement EventEmitter for PanelEvent if you need to emit events
impl EventEmitter<gpui_component::dock::PanelEvent> for EditorPanel {}

impl DockPanel for EditorPanel {
    fn panel_name(&self) -> &'static str {
        "EditorPanel" // Unique string identifier for this panel type
    }

    fn title(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child("Editor")
    }
}
