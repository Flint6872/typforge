pub mod code_editor;
pub mod diagnostics_panel;
pub mod editor_panel;
pub mod language_setup;
pub mod tabs;

pub use code_editor::CodeEditor;
pub use diagnostics_panel::DiagnosticsPanel;
pub use editor_panel::EditorPanel;
pub use language_setup::init_editor_languages;

//use gpui_kit::base::input::EditorState;
use gpui_kit::component::{ActiveTheme, input::EditorState};
use gpui_kit::{Context, EntityInputHandler as _, Window, *};

use std::{
    fs,
    ops::Range,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct FileContentUpdated {
    pub path: Option<std::path::PathBuf>,
    pub content: String,
}

struct TabDrag {
    pub from_index: usize,
}

struct DraggedTab {
    name: String,
}

impl Render for DraggedTab {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .bg(cx.theme().foreground)
            .px_3()
            .py_1()
            .border_1()
            .border_color(rgb(0x444444))
            .child(self.name.clone())
    }
}

pub trait EditorStateExt: Sized {
    fn replace_range_with_history(
        &mut self,
        range: Range<usize>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    );
}

impl EditorStateExt for EditorState {
    fn replace_range_with_history(
        &mut self,
        range: Range<usize>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = self.text();
        let start_utf16 = text.byte_to_utf16_idx(range.start);
        let end_utf16 = text.byte_to_utf16_idx(range.end);
        let range_utf16 = start_utf16..end_utf16;

        self.replace_text_in_range(Some(range_utf16), new_text, window, cx);
    }
}

#[derive(Clone)]
pub struct OpenedFile {
    pub path: PathBuf,
    pub editor_state: Entity<EditorState>,
    pub language: String,          // e.g., "rust", "markdown", "typst"
    pub has_unsaved_changes: bool, // Future: Track if content is modified
    pub lsp_version: i32,          // Track document version
    pub diagnostics: Vec<typst::diag::SourceDiagnostic>, // Store native diagnostics
    pub code_editor_entity: Entity<CodeEditor>,
    pub is_untitled: bool,
}

impl OpenedFile {
    /// Creates a new `OpenedFile` by reading content from the given path.
    pub fn new(path: PathBuf, window: &mut Window, cx: &mut App) -> Result<Self> {
        let content = fs::read_to_string(&path)?;
        let language = get_language_from_path(&path);

        let editor_state = cx.new(|cx| {
            let mut state = EditorState::new(window, cx)
                .language(language.clone())
                .soft_wrap(false)
                .line_number(true)
                .searchable(true)
                .default_value(content);

            // Ensure highlighter is explicitly initialized for typst
            state.set_highlighter(language.clone(), cx);
            state
        });

        // Create the CodeEditor entity and pass its InputState
        let code_editor_entity = cx.new(|_cx| {
            CodeEditor::new(
                editor_state.clone(),
                language.clone(),
                Vec::new(), // Initial empty diagnostics
            )
        });

        Ok(Self {
            path,
            editor_state,
            language,
            has_unsaved_changes: false,
            lsp_version: 0,
            diagnostics: Vec::new(),
            code_editor_entity,
            is_untitled: false,
        })
    }

    /// Returns a display name for the tab (e.g., "main.rs").
    pub fn tab_name(&self) -> String {
        if self.path == Path::new("typforge://package-manager") {
            return "Package Manager".to_string();
        }

        self.path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("untitled")
            .to_string()
    }
}

// Helper to guess language from file extension
fn get_language_from_path(path: &Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| match ext.to_lowercase().as_str() {
            "html" | "htm" => "html",
            "rs" => "rust",
            "md" => "markdown",
            "toml" => "toml",
            "typ" => "typst",
            _ => "text",
        })
        .unwrap_or("text")
        .to_string()
}

impl EventEmitter<InputEvent> for EditorState {}

#[derive(Clone)]
pub enum InputEvent {
    Change,
    PressEnter { secondary: bool },
    Focus,
    Blur,
}
