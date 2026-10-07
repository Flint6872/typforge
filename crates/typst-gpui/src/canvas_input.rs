use crate::caret_geometry::{
    BoundaryAction, CaretAffinity, CaretGeometryEngine, HorizontalDirection, LineBoundary,
    VerticalDirection, VisualCaret,
};
use crate::typst_element::HitMap;
use gpui::{App, ClipboardItem, KeyDownEvent, Pixels, Point, point, px};

pub enum CanvasAction {
    None,
    Undo,
    Redo,
}

pub struct CanvasInputResult {
    pub new_source: Option<String>,
    pub new_cursor: usize,
    pub new_selection: Option<std::ops::Range<usize>>,
    pub action: CanvasAction,
    pub handled: bool,
}

pub struct CanvasInputHandler;

impl CanvasInputHandler {
    /// Builds a VisualCaret from the current byte cursor offset.
    pub fn build_visual_caret(cursor_offset: usize, hit_map: &HitMap) -> VisualCaret {
        if hit_map.glyphs.is_empty() {
            return VisualCaret::default();
        }

        // 1. Find glyph containing or matching the cursor offset
        let mut best_idx = 0;
        let mut affinity = CaretAffinity::Preceding;

        for (idx, g) in hit_map.glyphs.iter().enumerate() {
            if cursor_offset >= g.byte_offset && cursor_offset < g.byte_offset + g.byte_len {
                best_idx = idx;
                affinity = CaretAffinity::Preceding;
                break;
            } else if cursor_offset == g.byte_offset + g.byte_len {
                best_idx = idx;
                affinity = CaretAffinity::Following;
            } else if g.byte_offset > cursor_offset {
                if idx > 0 && affinity == CaretAffinity::Following {
                    // Stay with previous glyph trailing edge
                } else {
                    best_idx = idx;
                    affinity = CaretAffinity::Preceding;
                }
                break;
            }
        }

        let (span, source_offset) = if let Some(g) = hit_map.glyphs.get(best_idx) {
            let offset = if affinity == CaretAffinity::Following {
                g.byte_offset + g.byte_len
            } else {
                g.byte_offset
            };
            (g.span, offset)
        } else {
            (typst::syntax::Span::detached(), cursor_offset)
        };

        VisualCaret {
            frame_path: Vec::new(),
            glyph_index: best_idx,
            affinity,
            span,
            source_offset,
            sticky_x: None,
        }
    }

    /// Handles keyboard navigation, text insertion, deletions, and clipboard commands.
    pub fn handle_key_down(
        event: &KeyDownEvent,
        current_source: &str,
        cursor_offset: usize,
        selection: Option<std::ops::Range<usize>>,
        hit_map: &HitMap,
        cx: &mut App,
    ) -> CanvasInputResult {
        let key = event.keystroke.key.as_str();
        let key_lower = key.to_lowercase();
        let is_cmd = event.keystroke.modifiers.platform || event.keystroke.modifiers.control;
        let is_shift = event.keystroke.modifiers.shift;
        let is_alt = event.keystroke.modifiers.alt;

        let caret = Self::build_visual_caret(cursor_offset, hit_map);

        // --- 1. CLIPBOARD & SELECTION SHORTCUTS ---
        if is_cmd {
            match key_lower.as_str() {
                "a" => {
                    return CanvasInputResult {
                        new_source: None,
                        new_cursor: current_source.len(),
                        new_selection: Some(0..current_source.len()),
                        action: CanvasAction::None,
                        handled: true,
                    };
                }
                "z" => {
                    return CanvasInputResult {
                        new_source: None,
                        new_cursor: cursor_offset,
                        new_selection: selection,
                        action: if is_shift {
                            CanvasAction::Redo
                        } else {
                            CanvasAction::Undo
                        },
                        handled: true,
                    };
                }
                "y" => {
                    return CanvasInputResult {
                        new_source: None,
                        new_cursor: cursor_offset,
                        new_selection: selection,
                        action: CanvasAction::Redo,
                        handled: true,
                    };
                }
                "c" => {
                    if let Some(ref sel) = selection {
                        if let Some(selected_slice) = current_source.get(sel.clone()) {
                            cx.write_to_clipboard(ClipboardItem::new_string(
                                selected_slice.to_string(),
                            ));
                        }
                    }
                    return CanvasInputResult {
                        new_source: None,
                        new_cursor: cursor_offset,
                        new_selection: selection,
                        action: CanvasAction::None,
                        handled: true,
                    };
                }
                "x" => {
                    if let Some(ref sel) = selection {
                        if let Some(selected_slice) = current_source.get(sel.clone()) {
                            cx.write_to_clipboard(ClipboardItem::new_string(
                                selected_slice.to_string(),
                            ));
                            let mut text = current_source.to_string();
                            text.replace_range(sel.clone(), "");
                            let new_cursor = sel.start;
                            return CanvasInputResult {
                                new_source: Some(text),
                                new_cursor,
                                new_selection: None,
                                action: CanvasAction::None,
                                handled: true,
                            };
                        }
                    }
                }
                "v" => {
                    if let Some(clipboard) = cx.read_from_clipboard() {
                        if let Some(pasted_str) = clipboard.text() {
                            let (new_text, new_cursor) = if let Some(ref sel) = selection {
                                let mut text = current_source.to_string();
                                text.replace_range(sel.clone(), &pasted_str);
                                let cursor = sel.start + pasted_str.len();
                                (text, cursor)
                            } else {
                                let mut text = current_source.to_string();
                                text.insert_str(cursor_offset, &pasted_str);
                                let cursor = cursor_offset + pasted_str.len();
                                (text, cursor)
                            };

                            return CanvasInputResult {
                                new_source: Some(new_text),
                                new_cursor,
                                new_selection: None,
                                action: CanvasAction::None,
                                handled: true,
                            };
                        }
                    }
                }
                _ => {}
            }
        }

        // --- 2. 2D CARET & MODIFIER NAVIGATION ---
        match key_lower.as_str() {
            "left" | "arrowleft" => {
                let next_caret = if is_cmd {
                    CaretGeometryEngine::move_to_line_boundary(&caret, LineBoundary::Start, hit_map)
                } else if is_alt {
                    CaretGeometryEngine::move_by_word(
                        &caret,
                        HorizontalDirection::Left,
                        current_source,
                        hit_map,
                    )
                } else {
                    CaretGeometryEngine::move_horizontal(&caret, HorizontalDirection::Left, hit_map)
                };

                let new_cursor = next_caret.source_offset;
                let new_sel = if is_shift {
                    let anchor = selection.map(|s| s.start).unwrap_or(cursor_offset);
                    Some(anchor.min(new_cursor)..anchor.max(new_cursor))
                } else {
                    None
                };

                return CanvasInputResult {
                    new_source: None,
                    new_cursor,
                    new_selection: new_sel,
                    action: CanvasAction::None,
                    handled: true,
                };
            }
            "right" | "arrowright" => {
                let next_caret = if is_cmd {
                    CaretGeometryEngine::move_to_line_boundary(&caret, LineBoundary::End, hit_map)
                } else if is_alt {
                    CaretGeometryEngine::move_by_word(
                        &caret,
                        HorizontalDirection::Right,
                        current_source,
                        hit_map,
                    )
                } else {
                    CaretGeometryEngine::move_horizontal(
                        &caret,
                        HorizontalDirection::Right,
                        hit_map,
                    )
                };

                let new_cursor = next_caret.source_offset;
                let new_sel = if is_shift {
                    let anchor = selection.map(|s| s.start).unwrap_or(cursor_offset);
                    Some(anchor.min(new_cursor)..anchor.max(new_cursor))
                } else {
                    None
                };

                return CanvasInputResult {
                    new_source: None,
                    new_cursor,
                    new_selection: new_sel,
                    action: CanvasAction::None,
                    handled: true,
                };
            }
            "up" | "arrowup" => {
                let next_caret =
                    CaretGeometryEngine::move_vertical(&caret, VerticalDirection::Up, hit_map);
                let new_cursor = next_caret.source_offset;
                let new_sel = if is_shift {
                    let anchor = selection.map(|s| s.start).unwrap_or(cursor_offset);
                    Some(anchor.min(new_cursor)..anchor.max(new_cursor))
                } else {
                    None
                };

                return CanvasInputResult {
                    new_source: None,
                    new_cursor,
                    new_selection: new_sel,
                    action: CanvasAction::None,
                    handled: true,
                };
            }
            "down" | "arrowdown" => {
                let next_caret =
                    CaretGeometryEngine::move_vertical(&caret, VerticalDirection::Down, hit_map);
                let new_cursor = next_caret.source_offset;
                let new_sel = if is_shift {
                    let anchor = selection.map(|s| s.start).unwrap_or(cursor_offset);
                    Some(anchor.min(new_cursor)..anchor.max(new_cursor))
                } else {
                    None
                };

                return CanvasInputResult {
                    new_source: None,
                    new_cursor,
                    new_selection: new_sel,
                    action: CanvasAction::None,
                    handled: true,
                };
            }
            "home" => {
                let next_caret = CaretGeometryEngine::move_to_line_boundary(
                    &caret,
                    LineBoundary::Start,
                    hit_map,
                );
                let new_cursor = next_caret.source_offset;
                return CanvasInputResult {
                    new_source: None,
                    new_cursor,
                    new_selection: None,
                    action: CanvasAction::None,
                    handled: true,
                };
            }
            "end" => {
                let next_caret =
                    CaretGeometryEngine::move_to_line_boundary(&caret, LineBoundary::End, hit_map);
                let new_cursor = next_caret.source_offset;
                return CanvasInputResult {
                    new_source: None,
                    new_cursor,
                    new_selection: None,
                    action: CanvasAction::None,
                    handled: true,
                };
            }

            // --- 3. EDITING & CONTENT BLOCK DELETION ---
            "backspace" => {
                if let Some(ref sel) = selection {
                    let mut text = current_source.to_string();
                    text.replace_range(sel.clone(), "");
                    let new_cursor = sel.start;
                    return CanvasInputResult {
                        new_source: Some(text),
                        new_cursor,
                        new_selection: None,
                        action: CanvasAction::None,
                        handled: true,
                    };
                }

                // Check content block boundary protection
                match CaretGeometryEngine::handle_backspace_at_boundary(
                    current_source,
                    cursor_offset,
                ) {
                    BoundaryAction::SelectParentBlock(range) => {
                        return CanvasInputResult {
                            new_source: None,
                            new_cursor: range.end,
                            new_selection: Some(range),
                            action: CanvasAction::None,
                            handled: true,
                        };
                    }
                    BoundaryAction::AllowNormalDelete => {
                        if cursor_offset > 0 {
                            let mut text = current_source.to_string();
                            let prev_idx = current_source[..cursor_offset]
                                .char_indices()
                                .last()
                                .map(|(idx, _)| idx)
                                .unwrap_or(0);

                            text.replace_range(prev_idx..cursor_offset, "");
                            return CanvasInputResult {
                                new_source: Some(text),
                                new_cursor: prev_idx,
                                new_selection: None,
                                action: CanvasAction::None,
                                handled: true,
                            };
                        }
                    }
                }
            }
            "delete" => {
                if let Some(ref sel) = selection {
                    let mut text = current_source.to_string();
                    text.replace_range(sel.clone(), "");
                    return CanvasInputResult {
                        new_source: Some(text),
                        new_cursor: sel.start,
                        new_selection: None,
                        action: CanvasAction::None,
                        handled: true,
                    };
                } else if cursor_offset < current_source.len() {
                    let mut text = current_source.to_string();
                    let next_len = current_source[cursor_offset..]
                        .chars()
                        .next()
                        .map(|c| c.len_utf8())
                        .unwrap_or(1);
                    text.replace_range(cursor_offset..cursor_offset + next_len, "");
                    return CanvasInputResult {
                        new_source: Some(text),
                        new_cursor: cursor_offset,
                        new_selection: None,
                        action: CanvasAction::None,
                        handled: true,
                    };
                }
            }
            "enter" => {
                let mut text = current_source.to_string();
                if let Some(ref sel) = selection {
                    text.replace_range(sel.clone(), "\n");
                    return CanvasInputResult {
                        new_source: Some(text),
                        new_cursor: sel.start + 1,
                        new_selection: None,
                        action: CanvasAction::None,
                        handled: true,
                    };
                } else {
                    text.insert(cursor_offset, '\n');
                    return CanvasInputResult {
                        new_source: Some(text),
                        new_cursor: cursor_offset + 1,
                        new_selection: None,
                        action: CanvasAction::None,
                        handled: true,
                    };
                }
            }
            "space" | " " => {
                // Explicitly catch space
                let mut text = current_source.to_string();
                text.insert(cursor_offset, ' ');
                return CanvasInputResult {
                    new_source: Some(text),
                    new_cursor: cursor_offset + 1,
                    new_selection: None,
                    action: CanvasAction::None,
                    handled: true,
                };
            }
            _ => {
                // Printable character typing
                if !is_cmd && !key.is_empty() && key.chars().count() == 1 {
                    let typed_char = get_shifted_char(key, is_shift);
                    let mut text = current_source.to_string();
                    if let Some(ref sel) = selection {
                        text.replace_range(sel.clone(), &typed_char);
                        return CanvasInputResult {
                            new_source: Some(text),
                            new_cursor: sel.start + typed_char.len(),
                            new_selection: None,
                            action: CanvasAction::None,
                            handled: true,
                        };
                    } else {
                        text.insert_str(cursor_offset, &typed_char);
                        return CanvasInputResult {
                            new_source: Some(text),
                            new_cursor: cursor_offset + typed_char.len(),
                            new_selection: None,
                            action: CanvasAction::None,
                            handled: true,
                        };
                    }
                }
            }
        }

        CanvasInputResult {
            new_source: None,
            new_cursor: cursor_offset,
            new_selection: selection,
            action: CanvasAction::None,
            handled: false,
        }
    }

    /// Computes the pixel coordinate for the IME composition candidate popup.
    pub fn ime_popup_point(cursor_offset: usize, hit_map: &HitMap) -> Point<Pixels> {
        let caret = Self::build_visual_caret(cursor_offset, hit_map);
        if let Some((x, y, h)) = CaretGeometryEngine::caret_position_and_height(&caret, hit_map) {
            point(px(x), px(y + h))
        } else {
            point(px(0.0), px(0.0))
        }
    }
}

// Translates lower-case keys to upper-case/shifted characters when Shift is active.
fn get_shifted_char(key: &str, is_shift: bool) -> String {
    // Escape Typst special characters regardless of whether shift was resolved by GPUI or not
    if key == "#" {
        return "\\#".to_string();
    }
    if key == "$" {
        return "\\$".to_string();
    }
    if key == "*" {
        return "\\*".to_string();
    }
    if key == "_" {
        return "\\_".to_string();
    }

    if !is_shift {
        return key.to_string();
    }

    // Safely match on the first character if it exists
    match key.chars().next() {
        // If it's a lowercase letter, handle it first
        Some(c) if c.is_ascii_lowercase() => c.to_ascii_uppercase().to_string(),

        // Map the special shift characters (fallback for platforms where keys aren't pre-resolved)
        Some('1') => "!".to_string(),
        Some('2') => "@".to_string(),
        Some('3') => "\\#".to_string(),
        Some('4') => "\\$".to_string(),
        Some('5') => "%".to_string(),
        Some('6') => "^".to_string(),
        Some('7') => "&".to_string(),
        Some('8') => "\\*".to_string(),
        Some('9') => "(".to_string(),
        Some('0') => ")".to_string(),
        Some('-') => "Y_".to_string(),
        Some('=') => "+".to_string(),
        Some('[') => "{".to_string(),
        Some(']') => "}".to_string(),
        Some('\\') => "|".to_string(),
        Some(';') => ":".to_string(),
        Some('\'') => "\"".to_string(),
        Some(',') => "<".to_string(),
        Some('.') => ">".to_string(),
        Some('/') => "?".to_string(),
        //this one still needs some work, both ~ and ` not rendering properly thinking both will need to be escaped
        Some('`') => "//~".to_string(),

        // Fallback: If it's an empty string (None) or any other character, return original key
        _ => key.to_string(),
    }
}
