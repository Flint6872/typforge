use crate::typst_element::HitMap;
use typst::syntax::{LinkedNode, Side, Source, Span, SyntaxKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaretAffinity {
    Preceding,
    Following,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VisualCaret {
    pub frame_path: Vec<usize>,
    pub glyph_index: usize,
    pub affinity: CaretAffinity,
    pub span: Span,
    pub source_offset: usize,
    pub sticky_x: Option<f32>, // Preserves column position across vertical moves
}

impl Default for VisualCaret {
    fn default() -> Self {
        Self {
            frame_path: Vec::new(),
            glyph_index: 0,
            affinity: CaretAffinity::Preceding,
            span: Span::detached(),
            source_offset: 0,
            sticky_x: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorizontalDirection {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalDirection {
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineBoundary {
    Start,
    End,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryAction {
    /// Normal deletion can proceed inside the content block.
    AllowNormalDelete,
    /// Backspace was pressed at the very start of a content block.
    /// Returns the source range of the parent block to select instead of deleting bracket.
    SelectParentBlock(std::ops::Range<usize>),
}

pub struct CaretGeometryEngine;

impl CaretGeometryEngine {
    /// Computes the visual caret point (origin and height) for rendering or raycasting.
    pub fn caret_position_and_height(
        caret: &VisualCaret,
        hit_map: &HitMap,
    ) -> Option<(f32, f32, f32)> {
        let glyphs = &hit_map.glyphs;
        if glyphs.is_empty() {
            return None;
        }

        if let Some(glyph) = glyphs.get(caret.glyph_index) {
            let x = if caret.affinity == CaretAffinity::Following {
                glyph.bounds.left().as_f32() + glyph.bounds.size.width.as_f32()
            } else {
                glyph.bounds.left().as_f32()
            };
            let y = glyph.bounds.top().as_f32();
            let height = glyph.bounds.size.height.as_f32();
            Some((x, y, height))
        } else if let Some(last) = glyphs.last() {
            let x = last.bounds.left().as_f32() + last.bounds.size.width.as_f32();
            let y = last.bounds.top().as_f32();
            let height = last.bounds.size.height.as_f32();
            Some((x, y, height))
        } else {
            None
        }
    }

    /// Moves the caret 1 visual glyph left or right across visual text bounds.
    /// Resets `sticky_x` to `None`.
    pub fn move_horizontal(
        caret: &VisualCaret,
        direction: HorizontalDirection,
        hit_map: &HitMap,
    ) -> VisualCaret {
        let glyphs = &hit_map.glyphs;
        if glyphs.is_empty() {
            return caret.clone();
        }

        let mut next = caret.clone();
        next.sticky_x = None; // Reset column memory on horizontal move

        match direction {
            HorizontalDirection::Right => {
                if caret.glyph_index + 1 < glyphs.len() {
                    let next_idx = caret.glyph_index + 1;
                    if let Some(glyph) = glyphs.get(next_idx) {
                        next.glyph_index = next_idx;
                        next.source_offset = glyph.byte_offset;
                        next.span = glyph.span;
                        next.affinity = CaretAffinity::Preceding;
                    }
                } else if let Some(last) = glyphs.last() {
                    next.glyph_index = glyphs.len() - 1;
                    next.source_offset = last.byte_offset + last.byte_len;
                    next.span = last.span;
                    next.affinity = CaretAffinity::Following;
                }
            }
            HorizontalDirection::Left => {
                if caret.glyph_index > 0 {
                    let prev_idx = caret.glyph_index - 1;
                    if let Some(glyph) = glyphs.get(prev_idx) {
                        next.glyph_index = prev_idx;
                        next.source_offset = glyph.byte_offset;
                        next.span = glyph.span;
                        next.affinity = CaretAffinity::Preceding;
                    }
                } else if let Some(first) = glyphs.first() {
                    next.glyph_index = 0;
                    next.source_offset = first.byte_offset;
                    next.span = first.span;
                    next.affinity = CaretAffinity::Preceding;
                }
            }
        }

        next
    }

    /// Moves the caret vertically across lines using spatial raycasting.
    /// Captures and maintains `sticky_x` across consecutive vertical moves.
    pub fn move_vertical(
        caret: &VisualCaret,
        direction: VerticalDirection,
        hit_map: &HitMap,
    ) -> VisualCaret {
        let glyphs = &hit_map.glyphs;
        if glyphs.is_empty() {
            return caret.clone();
        }

        let (current_x, current_y, current_h) =
            match Self::caret_position_and_height(caret, hit_map) {
                Some(pos) => pos,
                None => return caret.clone(),
            };

        // Set or retain sticky column position
        let target_x = caret.sticky_x.unwrap_or(current_x);

        // Find candidate vertical line coordinates
        let mut target_line_y: Option<f32> = None;

        match direction {
            VerticalDirection::Down => {
                let mut best_above = f32::MAX;
                for g in glyphs {
                    let top = g.bounds.top().as_f32();
                    if top > current_y + (current_h * 0.5) && top < best_above {
                        best_above = top;
                    }
                }
                if best_above != f32::MAX {
                    target_line_y = Some(best_above);
                }
            }
            VerticalDirection::Up => {
                let mut best_below = f32::MIN;
                for g in glyphs {
                    let top = g.bounds.top().as_f32();
                    if top < current_y - (current_h * 0.5) && top > best_below {
                        best_below = top;
                    }
                }
                if best_below != f32::MIN {
                    target_line_y = Some(best_below);
                }
            }
        }

        let line_y = match target_line_y {
            Some(y) => y,
            None => return caret.clone(), // At document vertical boundary
        };

        // Perform spatial horizontal raycast on target line to find closest glyph
        let mut best_index = caret.glyph_index;
        let mut min_dist = f32::MAX;

        for (idx, g) in glyphs.iter().enumerate() {
            let top = g.bounds.top().as_f32();
            if (top - line_y).abs() < 5.0 {
                let left = g.bounds.left().as_f32();
                let right = left + g.bounds.size.width.as_f32();
                let center = left + (g.bounds.size.width.as_f32() / 2.0);

                let dist = if target_x >= left && target_x < right {
                    (center - target_x).abs()
                } else if target_x < left {
                    (left - target_x) + 1000.0
                } else {
                    (target_x - right) + 1000.0
                };

                if dist < min_dist {
                    min_dist = dist;
                    best_index = idx;
                }
            }
        }

        let target_glyph = match glyphs.get(best_index) {
            Some(g) => g,
            None => return caret.clone(),
        };

        VisualCaret {
            frame_path: caret.frame_path.clone(),
            glyph_index: best_index,
            affinity: CaretAffinity::Preceding,
            span: target_glyph.span,
            source_offset: target_glyph.byte_offset,
            sticky_x: Some(target_x), // Retain column memory
        }
    }

    /// Jumps the caret to the visual start or end of the current line.
    pub fn move_to_line_boundary(
        caret: &VisualCaret,
        boundary: LineBoundary,
        hit_map: &HitMap,
    ) -> VisualCaret {
        let glyphs = &hit_map.glyphs;
        if glyphs.is_empty() {
            return caret.clone();
        }

        let current_glyph = match glyphs.get(caret.glyph_index) {
            Some(g) => g,
            None => return caret.clone(),
        };

        let current_line_top = current_glyph.bounds.top().as_f32();
        let current_line_height = current_glyph.bounds.size.height.as_f32();
        let current_line_center_y = current_line_top + (current_line_height / 2.0);

        // Collect all glyph indices that strictly share this vertical band
        let mut line_indices = Vec::new();
        for (idx, g) in glyphs.iter().enumerate() {
            let g_top = g.bounds.top().as_f32();
            let g_bottom = g_top + g.bounds.size.height.as_f32();

            // If the current line's center point is inside this glyph's vertical bounds
            if current_line_center_y >= g_top && current_line_center_y <= g_bottom {
                line_indices.push(idx);
            }
        }

        if line_indices.is_empty() {
            return caret.clone();
        }

        let mut next = caret.clone();
        next.sticky_x = None;

        match boundary {
            LineBoundary::Start => {
                if let Some(&first_idx) = line_indices.first() {
                    if let Some(glyph) = glyphs.get(first_idx) {
                        next.glyph_index = first_idx;
                        next.source_offset = glyph.byte_offset;
                        next.span = glyph.span;
                        next.affinity = CaretAffinity::Preceding;
                    }
                }
            }
            LineBoundary::End => {
                if let Some(&last_idx) = line_indices.last() {
                    if let Some(glyph) = glyphs.get(last_idx) {
                        next.glyph_index = last_idx;
                        // The end of a line is the trailing edge of the last glyph
                        next.source_offset = glyph.byte_offset + glyph.byte_len;
                        next.span = glyph.span;
                        next.affinity = CaretAffinity::Following;
                    }
                }
            }
        }

        next
    }

    /// Jumps the caret by full words using Unicode character classification on the source text.
    pub fn move_by_word(
        caret: &VisualCaret,
        direction: HorizontalDirection,
        source_text: &str,
        hit_map: &HitMap,
    ) -> VisualCaret {
        let glyphs = &hit_map.glyphs;
        if glyphs.is_empty() {
            return caret.clone();
        }

        let mut next = caret.clone();
        next.sticky_x = None;

        let char_at_offset =
            |offset: usize| -> Option<char> { source_text.get(offset..)?.chars().next() };

        let is_word_char = |c: char| c.is_alphanumeric() || c == '_';

        match direction {
            HorizontalDirection::Right => {
                let mut idx = caret.glyph_index;
                let mut passed_word = false;

                // 1. Consume current word characters
                while idx + 1 < glyphs.len() {
                    let next_idx = idx + 1;
                    if let Some(g) = glyphs.get(next_idx) {
                        if let Some(c) = char_at_offset(g.byte_offset) {
                            if is_word_char(c) {
                                passed_word = true;
                                idx = next_idx;
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }

                // 2. Consume whitespace until the start of the next word
                while idx + 1 < glyphs.len() {
                    let next_idx = idx + 1;
                    if let Some(g) = glyphs.get(next_idx) {
                        if let Some(c) = char_at_offset(g.byte_offset) {
                            if c.is_whitespace() {
                                idx = next_idx;
                            } else {
                                // Land on first character of next word
                                idx = next_idx;
                                break;
                            }
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }

                if let Some(g) = glyphs.get(idx) {
                    next.glyph_index = idx;
                    next.source_offset = g.byte_offset;
                    next.span = g.span;
                    next.affinity = CaretAffinity::Preceding;
                }
            }
            HorizontalDirection::Left => {
                let mut idx = caret.glyph_index;

                // 1. Consume preceding whitespace
                while idx > 0 {
                    let prev_idx = idx - 1;
                    if let Some(g) = glyphs.get(prev_idx) {
                        if let Some(c) = char_at_offset(g.byte_offset) {
                            if c.is_whitespace() {
                                idx = prev_idx;
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }

                // 2. Consume word characters backwards to land at word start
                while idx > 0 {
                    let prev_idx = idx - 1;
                    if let Some(g) = glyphs.get(prev_idx) {
                        if let Some(c) = char_at_offset(g.byte_offset) {
                            if is_word_char(c) {
                                idx = prev_idx;
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }

                if let Some(g) = glyphs.get(idx) {
                    next.glyph_index = idx;
                    next.source_offset = g.byte_offset;
                    next.span = g.span;
                    next.affinity = CaretAffinity::Preceding;
                }
            }
        }

        next
    }

    /// Guards content block boundaries on deletion.
    /// If caret is at the start of a content block (e.g., right after `[`),
    /// backspace selects the parent block instead of deleting `[`.
    pub fn handle_backspace_at_boundary(source_text: &str, source_offset: usize) -> BoundaryAction {
        let source = Source::detached(source_text);
        let root = LinkedNode::new(source.root());

        if let Some(leaf) = root.leaf_at(source_offset, Side::Before) {
            let is_left_bracket = leaf.kind() == SyntaxKind::LeftBracket
                || leaf
                    .prev_sibling()
                    .map(|p| p.kind() == SyntaxKind::LeftBracket)
                    .unwrap_or(false);

            if is_left_bracket {
                // Traverse upward: prioritize outer FuncCall (#rect[...]) over raw ContentBlock ([...])
                let mut best_target: Option<std::ops::Range<usize>> = None;
                let mut current = leaf;
                while let Some(parent) = current.parent() {
                    if parent.kind() == SyntaxKind::ContentBlock {
                        best_target = Some(parent.range());
                    } else if parent.kind() == SyntaxKind::FuncCall {
                        let mut range = parent.range();
                        // In Typst markup mode, check for leading `#` token before the function call
                        if let Some(prev) = parent.prev_sibling() {
                            if prev.kind() == SyntaxKind::Hash {
                                range.start = prev.offset();
                            }
                        }
                        best_target = Some(range);
                        break;
                    }
                    current = parent.clone();
                }

                if let Some(range) = best_target {
                    return BoundaryAction::SelectParentBlock(range);
                }
            }
        }

        BoundaryAction::AllowNormalDelete
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typst_element::GlyphInfo;
    use gpui::{Bounds, point, size};

    fn dummy_multiline_hit_map() -> HitMap {
        let mut hit_map = HitMap::default();
        // Line 1: "Hi " -> 3 glyphs (offsets 0, 1, 2) at Y = 10.0
        for i in 0..3 {
            hit_map.push_glyph(GlyphInfo {
                bounds: Bounds::new(
                    point(gpui::px(i as f32 * 10.0), gpui::px(10.0)),
                    size(gpui::px(10.0), gpui::px(15.0)),
                ),
                byte_offset: i,
                byte_len: 1,
                span: Span::detached(),
            });
        }
        // Line 2: "World" -> 5 glyphs (offsets 3..8) at Y = 30.0
        for i in 0..5 {
            hit_map.push_glyph(GlyphInfo {
                bounds: Bounds::new(
                    point(gpui::px(i as f32 * 10.0), gpui::px(30.0)),
                    size(gpui::px(10.0), gpui::px(15.0)),
                ),
                byte_offset: 3 + i,
                byte_len: 1,
                span: Span::detached(),
            });
        }
        hit_map
    }

    #[test]
    fn test_horizontal_stepping() {
        let hit_map = dummy_multiline_hit_map();
        let mut caret = VisualCaret {
            glyph_index: 0,
            source_offset: 0,
            ..Default::default()
        };

        caret = CaretGeometryEngine::move_horizontal(&caret, HorizontalDirection::Right, &hit_map);
        assert_eq!(caret.glyph_index, 1);
        assert_eq!(caret.source_offset, 1);
        assert_eq!(caret.sticky_x, None);
    }

    #[test]
    fn test_vertical_sticky_x() {
        let hit_map = dummy_multiline_hit_map();
        let caret = VisualCaret {
            glyph_index: 2, // ' ' on Line 1 (x: 20..30)
            source_offset: 2,
            ..Default::default()
        };

        let down_caret =
            CaretGeometryEngine::move_vertical(&caret, VerticalDirection::Down, &hit_map);
        assert_eq!(down_caret.glyph_index, 5); // Rightmost glyph corresponding column on Line 2
        assert_eq!(down_caret.sticky_x, Some(20.0));

        let up_caret =
            CaretGeometryEngine::move_vertical(&down_caret, VerticalDirection::Up, &hit_map);
        assert_eq!(up_caret.glyph_index, 2);
        assert_eq!(up_caret.sticky_x, Some(20.0));
    }

    #[test]
    fn test_line_boundary_navigation() {
        let hit_map = dummy_multiline_hit_map();
        let caret = VisualCaret {
            glyph_index: 1, // "i" on line 1
            source_offset: 1,
            ..Default::default()
        };

        let start =
            CaretGeometryEngine::move_to_line_boundary(&caret, LineBoundary::Start, &hit_map);
        assert_eq!(start.glyph_index, 0);
        assert_eq!(start.source_offset, 0);

        let end = CaretGeometryEngine::move_to_line_boundary(&caret, LineBoundary::End, &hit_map);
        assert_eq!(end.glyph_index, 2);
        assert_eq!(end.source_offset, 3);
        assert_eq!(end.affinity, CaretAffinity::Following);
    }

    #[test]
    fn test_word_jumping() {
        let hit_map = dummy_multiline_hit_map();
        let source_text = "Hi World";

        let caret = VisualCaret {
            glyph_index: 0, // at 'H'
            source_offset: 0,
            ..Default::default()
        };

        let next_word = CaretGeometryEngine::move_by_word(
            &caret,
            HorizontalDirection::Right,
            source_text,
            &hit_map,
        );
        assert_eq!(next_word.glyph_index, 3);
        assert_eq!(next_word.source_offset, 3);
    }

    #[test]
    fn test_content_block_boundary_protection() {
        let source = "#rect[Hello]";
        let action = CaretGeometryEngine::handle_backspace_at_boundary(source, 6);
        match action {
            BoundaryAction::SelectParentBlock(range) => {
                assert_eq!(&source[range], "#rect[Hello]");
            }
            BoundaryAction::AllowNormalDelete => panic!("Expected SelectParentBlock"),
        }
    }
}
