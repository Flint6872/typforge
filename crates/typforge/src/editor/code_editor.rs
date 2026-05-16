use gpui::{Point, Styled, *};
use gpui_component::{ActiveTheme, h_flex, input::*, v_flex};
use typstography::{Position as LspPosition, gpui_bridge::map_diagnostics};

//#[derive(Clone)]
pub struct CodeEditor {
    id: ElementId,
    editor: Entity<InputState>,
    language: String,
    height: Option<DefiniteLength>,
    font_size: Option<Pixels>,
    line_height: Option<Pixels>,

    // Listeners passed from the parent to handle LSP logic (Hover, etc.)
    on_mouse_move: Option<Box<dyn Fn(&MouseMoveEvent, &mut Window, &mut App) + 'static>>,
    on_mouse_down: Option<Box<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>>,
}

impl Clone for CodeEditor {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            editor: self.editor.clone(),
            language: self.language.clone(),
            height: self.height.clone(),
            font_size: self.font_size.clone(),
            line_height: self.font_size.clone(),
            // Closures cannot be cloned, so they are reset to None in a clone
            on_mouse_move: None,
            on_mouse_down: None,
        }
    }
}

impl CodeEditor {
    pub fn new(
        editor_state: Entity<InputState>,
        language: String,
        _initial_diagnostics: Vec<typstography::Diagnostic>,
    ) -> Self {
        Self {
            id: ElementId::from("code-editor"),
            editor: editor_state,
            language,
            height: None,
            font_size: None,
            line_height: None,
            on_mouse_move: None,
            on_mouse_down: None,
        }
    }

    pub fn font_size(mut self, size: Pixels) -> Self {
        self.font_size = Some(size);
        self
    }

    /// Sets the line height of this element and its children.
    pub fn line_height(mut self, line_height: impl Into<Pixels>) -> Self {
        self.line_height = Some(line_height.into());
        self
    }

    pub fn h_full(mut self) -> Self {
        self.height = Some(relative(1.)); // Use relative(1.) for full height
        self
    }

    /// Attach a listener for mouse move events (used for Hover).
    pub fn on_mouse_move(
        mut self,
        listener: impl Fn(&MouseMoveEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_mouse_move = Some(Box::new(listener));
        self
    }

    /// Attach a listener for mouse down events (used to clear UI).
    pub fn on_mouse_down(
        mut self,
        listener: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_mouse_down = Some(Box::new(listener));
        self
    }

    pub fn set_language(&mut self, language: String, _window: &mut Window, cx: &mut Context<Self>) {
        self.language = language.clone();
        self.editor.update(cx, |editor, cx| {
            editor.set_highlighter(language, cx);
        });
    }

    pub fn set_diagnostics(
        &mut self,
        lsp_diagnostics: Vec<typstography::Diagnostic>,
        cx: &mut Context<Self>,
    ) {
        self.editor.update(cx, |input_state, input_cx| {
            if let Some(diagnostic_set) = input_state.diagnostics_mut() {
                diagnostic_set.clear();
                // Use the bridge to convert 0.94 diagnostics to GPUI-compatible ones
                let gpui_diagnostics = map_diagnostics(lsp_diagnostics);
                diagnostic_set.extend(gpui_diagnostics);
            }
            input_cx.notify();
        });
    }

    /// Converts a screen position (Pixels) to an LSP-compatible text position (line, character).
    /// This implementation uses only the public API of InputState to perform the conversion.
    pub fn screen_to_lsp_position(
        &self,
        screen_position: Point<Pixels>,
        cx: &App,
    ) -> Option<LspPosition> {
        let input_state = self.editor.read(cx);
        let text = input_state.text();

        let visible_range = input_state.visible_row_range()?;

        for row in visible_range {
            // Cast `row` to `u32` for gpui_component::input::Position constructor
            let row_start = text.position_to_offset(&gpui_component::input::Position {
                line: row as u32,
                character: 0,
            });
            let next_row_start = text.position_to_offset(&gpui_component::input::Position {
                line: (row + 1) as u32,
                character: 0,
            });

            if let Some(line_bounds) = input_state.range_to_bounds(&(row_start..next_row_start)) {
                if screen_position.y >= line_bounds.top()
                    && screen_position.y <= line_bounds.bottom()
                {
                    let mut low = row_start;
                    let mut high = if next_row_start > row_start {
                        next_row_start - 1
                    } else {
                        row_start
                    };
                    let mut best_offset = row_start;

                    while low <= high {
                        let mid = (low + high) / 2;
                        if let Some(char_bounds) = input_state.range_to_bounds(&(mid..mid + 1)) {
                            if screen_position.x < char_bounds.left() {
                                high = mid.saturating_sub(1);
                            } else if screen_position.x > char_bounds.right() {
                                low = mid + 1;
                                best_offset = mid + 1;
                            } else {
                                best_offset = mid;
                                break;
                            }
                        } else {
                            high = mid.saturating_sub(1);
                        }
                    }

                    let pos = text.offset_to_position(best_offset);
                    return Some(LspPosition {
                        line: pos.line as u32,
                        character: pos.character as u32,
                    });
                }
            }
        }
        None
    }
}

impl Render for CodeEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut input = Input::new(&self.editor)
            .bordered(true)
            .h_full()
            .appearance(false);

        if let Some(size) = self.font_size {
            input = input.text_size(size);
        }

        if let Some(lh) = self.line_height {
            input = input.line_height(lh);
        }

        let mut input_container = div().id("input-container").size_full();

        if let Some(listener) = self.on_mouse_move.take() {
            input_container = input_container.on_mouse_move(listener);
        }

        if let Some(listener) = self.on_mouse_down.take() {
            input_container = input_container.on_any_mouse_down(listener);
        }

        v_flex()
            .id(self.id.clone())
            .gap_3()
            .child(
                h_flex()
                    .gap_2()
                    .child("Language:")
                    .child(div().child(self.language.clone())),
            )
            .child(div().flex_grow().child(input))
    }
}

impl IntoElement for CodeEditor {
    type Element = AnyElement;

    fn into_element(self) -> Self::Element {
        let mut input = Input::new(&self.editor).bordered(true).h_full();

        // Apply font size to the Input component if provided
        if let Some(size) = self.font_size {
            input = input.text_size(size);
        }

        if let Some(lh) = self.line_height {
            input = input.line_height(lh);
        }

        let mut root = v_flex()
            .id(self.id)
            .gap_3()
            .child(
                h_flex()
                    .gap_2()
                    .child("Language:")
                    .child(div().child(self.language.clone())),
            )
            .child(div().flex_grow().child(input));

        if let Some(height) = self.height {
            root = root.h(height);
        } else {
            root = root.h_full();
        }

        root.into_any_element()
    }
}
