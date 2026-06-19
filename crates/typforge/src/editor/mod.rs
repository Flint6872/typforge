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

use typforge_core::intel::Tooltip;

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
    pub language: String,          // e.g., "rust", "markdown", "typst"
    pub has_unsaved_changes: bool, // Future: Track if content is modified
    pub lsp_version: i32,          // Track document version
    pub diagnostics: Vec<typst::diag::SourceDiagnostic>, // Store native diagnostics
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
            lsp_version: 0,
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

impl EventEmitter<InputEvent> for InputState {}

#[derive(Clone)]
pub enum InputEvent {
    Change,
    PressEnter { secondary: bool },
    Focus,
    Blur,
}

// Helper to render the hover popup using our new Tooltip enum
fn render_hover_popup<W: typst::World + typforge_core::IdeWorld + 'static>(
    tooltip: &Tooltip,
    screen_pos: Point<Pixels>,
    _cx: &mut Context<EditorPanel<W>>,
) -> impl IntoElement {
    let content = match tooltip {
        Tooltip::Text(text) => text.to_string(),
        Tooltip::Code(code) => format!("```typst\n{}\n```", code),
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
    parent_element: AnyElement,
    children: impl Iterator<Item = AnyElement>,
) -> AnyElement {
    let mut elements: Vec<AnyElement> = vec![parent_element];
    elements.extend(children);
    div().children(elements).into_any_element()
}
