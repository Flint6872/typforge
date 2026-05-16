pub mod code_editor;
pub mod editor_panel;
pub mod tabs;

pub use code_editor::CodeEditor;
pub use editor_panel::EditorPanel;

use gpui::*;
use gpui_component::{ActiveTheme, input::InputState};

use std::{
    fs,
    path::{Path, PathBuf},
};

// Use types from typstography (0.94) for the backend communication
use typstography::{
    Diagnostic as LspDiagnostic, Hover, HoverContents, MarkedString, MarkupContent, Url,
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

#[derive(Clone)]
pub struct OpenedFile {
    pub path: PathBuf,
    pub editor_state: Entity<InputState>,
    pub language: String,                // e.g., "rust", "markdown", "typst"
    pub has_unsaved_changes: bool,       // Future: Track if content is modified
    pub lsp_version: i32,                // Track LSP document version
    pub diagnostics: Vec<LspDiagnostic>, // Store diagnostics for this file
    pub code_editor_entity: Entity<CodeEditor>,
}

impl OpenedFile {
    /// Creates a new `OpenedFile` by reading content from the given path.
    pub fn new(path: PathBuf, window: &mut Window, cx: &mut App) -> Result<Self> {
        let content = fs::read_to_string(&path)?;
        let language = get_language_from_path(&path);

        let editor_state = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor(language.clone())
                .multi_line(true)
                .soft_wrap(false)
                .line_number(true)
                .searchable(true)
                .default_value(content) // Load file content
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
            lsp_version: 0, // Start with version 0
            diagnostics: Vec::new(),
            code_editor_entity,
        })
    }

    /// Returns a display name for the tab (e.g., "main.rs").
    pub fn tab_name(&self) -> String {
        self.path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("untitled")
            .to_string()
    }

    pub fn uri(&self) -> Url {
        Url::from_file_path(&self.path).expect("Failed to convert path to URL")
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
            "typ" => "typst", // Assuming 'typst' is a recognized highlighter
            _ => "text",
        })
        .unwrap_or("text")
        .to_string()
}

impl EventEmitter<InputEvent> for InputState {}

#[derive(Clone)]
pub enum InputEvent {
    Change,
    PressEnter { secondary: bool },
    Focus,
    Blur,
}

// NEW: Helper to render the hover popup
fn render_hover_popup(
    hover: &Hover,
    screen_pos: Point<Pixels>,
    _cx: &mut Context<EditorPanel>,
) -> impl IntoElement {
    let content = match &hover.contents {
        HoverContents::Markup(MarkupContent { value, .. }) => value.clone(),
        HoverContents::Scalar(ms) => match ms {
            MarkedString::String(s) => s.clone(),
            MarkedString::LanguageString(ls) => ls.value.clone(),
        },
        HoverContents::Array(arr) => arr
            .iter()
            .map(|ms| match ms {
                MarkedString::String(s) => s.clone(),
                MarkedString::LanguageString(ls) => ls.value.clone(),
            })
            .collect::<Vec<_>>()
            .join("\n---\n"),
    };

    div()
        .bg(rgb(0x282C34)) // Dark background
        .text_color(rgb(0xABB2BF)) // Light text
        .p_2()
        .rounded_md()
        .border_1()
        .border_color(rgb(0x3E4452))
        .absolute()
        .left(screen_pos.x)
        .top(screen_pos.y + px(20.0)) // Offset slightly below mouse
        .w(px(400.0)) // Max width
        .child(content)
}

// Helper to render an element and then other elements on top in a stack.
fn element_to_parent_stack(
    mut parent_element: AnyElement,
    children: impl Iterator<Item = AnyElement>,
) -> AnyElement {
    let mut elements: Vec<AnyElement> = vec![parent_element];
    elements.extend(children);
    div().children(elements).into_any_element()
}
