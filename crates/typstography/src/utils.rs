use tower_lsp::lsp_types::{Position, Range};
use typst::World;
use typst::syntax::{Source, Span};

pub fn byte_to_lsp_position(source: &Source, byte_offset: usize) -> Option<Position> {
    let line = source.lines().byte_to_line(byte_offset)?;
    let column = source.lines().byte_to_column(byte_offset)?;

    Some(Position {
        line: line as u32,
        character: column as u32,
    })
}

pub fn typst_span_to_lsp_range(world: &dyn World, span: Span) -> Range {
    let file_id = match span.id() {
        Some(id) => id,
        None => return Range::default(),
    };

    match world.source(file_id) {
        Ok(source) => {
            let byte_range = match source.range(span) {
                Some(range) => range,
                None => return Range::default(),
            };

            let start_pos = byte_to_lsp_position(&source, byte_range.start);
            let end_pos = byte_to_lsp_position(&source, byte_range.end);

            match (start_pos, end_pos) {
                (Some(start), Some(end)) => Range { start, end },
                _ => Range::default(),
            }
        }
        Err(_) => Range::default(),
    }
}
