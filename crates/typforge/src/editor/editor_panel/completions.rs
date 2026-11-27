use gpui_kit::component::input::CompletionProvider;
use gpui_kit::{App, Task, Window}; // Added App, AsyncApp
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit,
    InsertTextFormat, Position, Range, TextEdit,
};
use ropey::{LineType, Rope};
use typastry::intel::{CompletionKind, get_enhanced_completions};
use typst_gpui::TypstGpuiWorld;

pub struct TypstCompletionProvider {
    world: Box<dyn TypstGpuiWorld>,
}

impl TypstCompletionProvider {
    pub fn new(world: Box<dyn TypstGpuiWorld>) -> Self {
        Self { world }
    }
}

impl CompletionProvider for TypstCompletionProvider {
    fn is_completion_trigger(&self, _offset: usize, new_text: &str, _cx: &mut App) -> bool {
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
        cx: &mut App,
    ) -> Task<anyhow::Result<CompletionResponse>> {
        // Use try_clone instead of clone()
        let mut world = self.world.try_clone().expect("World must be clonable");
        let content = rope.to_string();
        let rope_clone = rope.clone();

        cx.background_executor().spawn(async move {
            world.set_source(content);
            let main_id = world.main();

            let items = if let Ok(source) = world.source(main_id) {
                // world.as_ref() now works because TypstGpuiWorld requires IdeWorld
                let resp = get_enhanced_completions(world.as_ide(), None, &source, offset);

                let rope_str = rope_clone.to_string();
                let start_pos = offset_to_lsp_position(&rope_clone, resp.replace_range.start);
                let end_pos = offset_to_lsp_position(&rope_clone, offset);

                resp.items
                    .into_iter()
                    .map(|c| {
                        let kind = match c.kind {
                            CompletionKind::Func => CompletionItemKind::FUNCTION,
                            CompletionKind::Type => CompletionItemKind::CLASS,
                            CompletionKind::Param => CompletionItemKind::PROPERTY,
                            CompletionKind::Constant => CompletionItemKind::CONSTANT,
                            CompletionKind::Symbol => CompletionItemKind::VALUE,
                            CompletionKind::Unit => CompletionItemKind::UNIT,
                            CompletionKind::Package => CompletionItemKind::MODULE,
                            CompletionKind::Text => CompletionItemKind::TEXT,
                        };

                        let replacement_text = if c.kind == CompletionKind::Package {
                            if rope_str.as_bytes().get(offset) == Some(&b'"') {
                                c.apply
                            } else {
                                format!("{}\"", c.apply)
                            }
                        } else {
                            c.apply
                        };

                        CompletionItem {
                            label: c.label,
                            kind: Some(kind),
                            text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                                range: Range {
                                    start: start_pos,
                                    end: end_pos,
                                },
                                new_text: replacement_text,
                            })),
                            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                            detail: c.detail,
                            ..Default::default()
                        }
                    })
                    .collect::<Vec<CompletionItem>>() // Explicit type to help inference
            } else {
                Vec::new()
            };

            Ok(CompletionResponse::Array(items))
        })
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
