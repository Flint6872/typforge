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

            let trigger_offset = get_trigger_offset(rope, cursor);
            let start_pos = offset_to_lsp_position(rope, trigger_offset);
            let end_pos = offset_to_lsp_position(rope, cursor);

            let typed_prefix = if cursor > trigger_offset + 1 {
                let prefix_slice = rope.slice((trigger_offset + 1)..cursor);
                prefix_slice.to_string().to_lowercase()
            } else {
                String::new()
            };

            completions
                .into_iter()
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

                    // Convert Typst custom snippet syntax to valid LSP snippet syntax
                    let apply_text = convert_typst_snippet_to_lsp(&raw_apply_text);

                    let kind = match c.kind {
                        CompletionKind::Func => CompletionItemKind::FUNCTION,
                        CompletionKind::Type => CompletionItemKind::CLASS,
                        CompletionKind::Param => CompletionItemKind::PROPERTY,
                        CompletionKind::Constant => CompletionItemKind::CONSTANT,
                        CompletionKind::Symbol(_) => CompletionItemKind::VALUE,
                        _ => CompletionItemKind::TEXT,
                    };

                    let insert_text_format = if apply_text.contains('$') {
                        Some(InsertTextFormat::SNIPPET)
                    } else {
                        Some(InsertTextFormat::PLAIN_TEXT)
                    };

                    let replacement_text = format!("#{}", apply_text);

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
        new_text
            .chars()
            .any(|c| c.is_alphanumeric() || c == '#' || c == '.' || c == '(' || c == ',')
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
                // E.g. list(${}) -> list($1) -> Places cursor inside ()
                result.push_str(&format!(" "));

                tab_index += 1;
            } else if placeholder.contains(':') {
                // Already has standard format, keep it
                result.push_str(&format!("${{{}}}", placeholder));
            } else {
                // E.g. text(${body}) -> text(${1:body}) -> Highlights 'body' as a tab-stop
                result.push_str(&format!("${{{}:{}}}", tab_index, placeholder));
                tab_index += 1;
            }
        } else {
            result.push(c);
        }
    }
    result
}

fn get_trigger_offset(rope: &Rope, cursor: usize) -> usize {
    let mut offset = cursor;
    while offset > 0 {
        offset -= 1;
        if rope.char(offset) == '#' {
            return offset;
        }
        let c = rope.char(offset);
        if c.is_whitespace() || c == '\n' || c == '\r' {
            break;
        }
    }
    cursor.saturating_sub(1)
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
