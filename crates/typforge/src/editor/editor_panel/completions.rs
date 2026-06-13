// crates/typforge/src/editor/editor_panel/completions.rs

use gpui::{Context, Task, Window};
use gpui_component::input::{CompletionProvider, InputState};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit,
    InsertTextFormat, Position, Range, TextEdit,
};
use parking_lot::Mutex;
use ropey::{LineType, Rope};
use std::sync::Arc;
use typforge_core::intel::{CompletionKind, get_completions};

pub struct TypstCompletionProvider<W: typst::World + typforge_core::IdeWorld + 'static> {
    shared_world: Arc<Mutex<W>>,
}

impl<W: typst::World + typforge_core::IdeWorld + 'static> TypstCompletionProvider<W> {
    pub fn new(shared_world: Arc<Mutex<W>>) -> Self {
        Self { shared_world }
    }

    fn fetch_completions(
        &self,
        world_mutex: Arc<Mutex<W>>,
        rope: &Rope,
        cursor: usize,
    ) -> CompletionResponse {
        let world = world_mutex.lock();
        let main_id = world.main();

        let items = if let Ok(source) = world.source(main_id) {
            let completions = get_completions(&*world, None, &source, cursor, false);

            // 1. Get context-aware trigger details
            let (trigger_offset, is_hash_command) = get_trigger_info(rope, cursor);
            let start_pos = offset_to_lsp_position(rope, trigger_offset);
            let end_pos = offset_to_lsp_position(rope, cursor);

            // 2. Fetch the prefix currently typed after the trigger boundary
            let typed_prefix = if cursor > trigger_offset {
                let start = if is_hash_command {
                    trigger_offset + 1
                } else {
                    trigger_offset
                };
                if cursor > start {
                    let prefix_slice = rope.slice(start..cursor);
                    prefix_slice.to_string().to_lowercase()
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            completions
                .into_iter()
                // --- FILTER SUGGESTIONS DYNAMICALLY ---
                .filter(|c| {
                    if typed_prefix.is_empty() {
                        true
                    } else {
                        c.label
                            .to_string()
                            .to_lowercase()
                            .starts_with(&typed_prefix)
                    }
                })
                .map(|c| {
                    let label = c.label.to_string();
                    let raw_apply_text = c
                        .apply
                        .as_ref()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| label.clone());

                    // Simplify autocomplete text to raw plain text:
                    let apply_text = if raw_apply_text.contains('(') {
                        // E.g. "text(${body})" or "text()" -> "text()"
                        let base = raw_apply_text.split('(').next().unwrap_or(&label);
                        format!("{}()", base)
                    } else {
                        // E.g. "fill: ${}" -> "fill: "
                        raw_apply_text.replace("${}", "").replace("${1:}", "")
                    };

                    let kind = match c.kind {
                        CompletionKind::Func => CompletionItemKind::FUNCTION,
                        CompletionKind::Type => CompletionItemKind::CLASS,
                        CompletionKind::Param => CompletionItemKind::PROPERTY,
                        CompletionKind::Constant => CompletionItemKind::CONSTANT,
                        CompletionKind::Symbol(_) => CompletionItemKind::VALUE,
                        _ => CompletionItemKind::TEXT,
                    };

                    // Force PLAIN_TEXT to avoid literal $1 or ${}
                    let insert_text_format = Some(InsertTextFormat::PLAIN_TEXT);

                    let replacement_text = if is_hash_command {
                        format!("#{}", apply_text)
                    } else {
                        apply_text
                    };

                    let text_edit = CompletionTextEdit::Edit(TextEdit {
                        range: Range {
                            start: start_pos,
                            end: end_pos,
                        },
                        new_text: replacement_text,
                    });

                    CompletionItem {
                        label,
                        kind: Some(kind),
                        text_edit: Some(text_edit),
                        insert_text_format,
                        ..Default::default()
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        CompletionResponse::Array(items)
    }
}

impl<W: typst::World + typforge_core::IdeWorld + typst_gpui::TypstGpuiWorld + 'static>
    CompletionProvider for TypstCompletionProvider<W>
{
    fn is_completion_trigger(
        &self,
        _offset: usize,
        new_text: &str,
        _cx: &mut Context<InputState>,
    ) -> bool {
        // Trigger on `#` and keep updating the completion query as alphanumeric characters or code delimiters are typed
        new_text == "#"
            || new_text == "("
            || new_text == ","
            || new_text.chars().any(|c| c.is_alphanumeric() || c == '_')
    }

    fn completions(
        &self,
        rope: &Rope,
        offset: usize,
        _trigger: CompletionContext,
        _window: &mut Window,
        _cx: &mut Context<InputState>,
    ) -> Task<anyhow::Result<CompletionResponse>> {
        let world = self.shared_world.clone();

        let content = rope.to_string();
        {
            let mut world_lock = world.lock();
            world_lock.set_source(content);
        }

        Task::ready(Ok(self.fetch_completions(world, rope, offset)))
    }
}

/// Translates Typst snippet format (e.g., `${body}`) to LSP format (e.g., `${1:body}`)
fn convert_typst_snippet_to_lsp(snippet: &str) -> String {
    // Case 1: Simple function call (e.g., text() -> text($1))
    if snippet.ends_with("()") {
        return snippet.replace("()", "($1)");
    }

    // Case 2: Snippets that shouldn't be indexed (e.g. fill: )
    // We detect if the Typst snippet is just a placeholder (like ${})
    // and convert it to a plain string or a simple stop.

    let mut result = String::new();
    let mut chars = snippet.chars().peekable();
    let mut tab_index = 1;

    while let Some(c) = chars.next() {
        if c == '$' && chars.peek() == Some(&'{') {
            chars.next(); // Consume '{'
            let mut placeholder = String::new();
            while let Some(&inner_c) = chars.peek() {
                if inner_c == '}' {
                    chars.next(); // Consume '}'
                    break;
                }
                placeholder.push(chars.next().unwrap());
            }

            if placeholder.is_empty() {
                // If the placeholder is empty (like ${}), it's just a cursor position
                result.push_str(&format!("${}", tab_index));
                tab_index += 1;
            } else {
                // This handles your #text(fill: ${}) request.
                // By returning just the placeholder text, it won't have $1 inside.
                result.push_str(&placeholder);
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Scans backwards to find the context-aware trigger boundary.
/// Returns: (trigger_byte_offset, is_hash_command)
fn get_trigger_info(rope: &Rope, cursor: usize) -> (usize, bool) {
    if cursor == 0 {
        return (0, false);
    }

    // 1. Find the start of the current alphanumeric word/identifier
    let mut word_start = cursor;
    while word_start > 0 {
        let prev_c = rope.char(word_start - 1);
        if prev_c.is_alphanumeric() || prev_c == '_' || prev_c == '-' {
            word_start -= 1;
        } else {
            break;
        }
    }

    // 2. Check if the character immediately preceding the word is '#'
    if word_start > 0 && rope.char(word_start - 1) == '#' {
        // This is a top-level command call (e.g., `#text` or `#rect`)
        return (word_start - 1, true);
    }

    // 3. Otherwise, we are completing a parameter/variable inside code mode (e.g., `fil` in `text(fil)`)
    (word_start, false)
}

fn offset_to_lsp_position(rope: &Rope, offset: usize) -> Position {
    let offset_clamped = offset.min(rope.len());
    let line = rope.byte_to_line_idx(offset_clamped, LineType::LF);
    let line_start_byte = rope.line_to_byte_idx(line, LineType::LF);
    let slice = rope.slice(line_start_byte..offset_clamped);
    let char_offset: usize = slice.chars().map(|c| c.len_utf16()).sum();
    Position {
        line: line as u32,
        character: char_offset as u32,
    }
}
