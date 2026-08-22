use gpui::{Context, Task, Window};
use gpui_component::input::{CompletionProvider, InputState};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit,
    InsertTextFormat, Position, Range, TextEdit,
};
use parking_lot::Mutex;
use ropey::{LineType, Rope};
use std::sync::Arc;
use typastry::intel::{CompletionKind, get_enhanced_completions, get_trigger_info};

pub struct TypstCompletionProvider<W: typst::World + typastry::IdeWorld + 'static> {
    shared_world: Arc<Mutex<W>>,
}

impl<W: typst::World + typastry::IdeWorld + 'static> TypstCompletionProvider<W> {
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
            // 1. Fetch filtered, enhanced, and coached suggestions directly from typastry
            let enhanced_completions = get_enhanced_completions(&*world, None, &source, cursor);

            let rope_str = rope.to_string();
            let (trigger_offset, is_hash_command) = get_trigger_info(&rope_str, cursor);
            let start_pos = offset_to_lsp_position(rope, trigger_offset);
            let end_pos = offset_to_lsp_position(rope, cursor);

            // 2. Map clean, portable suggestions to editor-specific LSP types
            let list: Vec<CompletionItem> = enhanced_completions
                .into_iter()
                .map(|c| {
                    let kind = match c.kind {
                        CompletionKind::Func => CompletionItemKind::FUNCTION,
                        CompletionKind::Type => CompletionItemKind::CLASS,
                        CompletionKind::Param => CompletionItemKind::PROPERTY,
                        CompletionKind::Constant => CompletionItemKind::CONSTANT,
                        CompletionKind::Symbol => CompletionItemKind::VALUE,
                        CompletionKind::Unit => CompletionItemKind::UNIT,
                        CompletionKind::Text => CompletionItemKind::TEXT,
                    };

                    let replacement_text = if is_hash_command && c.kind != CompletionKind::Unit {
                        format!("#{}", c.apply)
                    } else {
                        c.apply
                    };

                    let text_edit = CompletionTextEdit::Edit(TextEdit {
                        range: Range {
                            start: start_pos,
                            end: end_pos,
                        },
                        new_text: replacement_text,
                    });

                    CompletionItem {
                        label: c.label,
                        kind: Some(kind),
                        text_edit: Some(text_edit),
                        insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                        detail: c.detail,
                        ..Default::default()
                    }
                })
                .collect();

            list
        } else {
            Vec::new()
        };

        CompletionResponse::Array(items)
    }
}

impl<W: typst::World + typastry::IdeWorld + typst_gpui::TypstGpuiWorld + 'static> CompletionProvider
    for TypstCompletionProvider<W>
{
    fn is_completion_trigger(
        &self,
        _offset: usize,
        new_text: &str,
        _cx: &mut Context<InputState>,
    ) -> bool {
        new_text == "#"
            || new_text == "("
            || new_text == ","
            || new_text
                .chars()
                .any(|c| c.is_alphanumeric() || c == '_' || c == '.')
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
