// crates/typforge/src/typst_element/frame_item_text.rs

use crate::typst_element::utils::{
    resolve_font_with_fallback, typst_color_to_gpui_hsla, typst_paint_to_gpui_hsla_from_paint,
};
use crate::typst_element::{GlyphInfo, HitMap};
use gpui::{App, Bounds, GlyphId, Pixels, Point, Window, size};
use typst_library::text::TextItem;

pub fn frame_item_text(
    text_item: &TextItem,
    item_absolute_origin_gpui: Point<Pixels>,
    scale_factor: f32,
    window: &mut Window,
    cx: &mut App,
    span_resolver: &Option<
        std::sync::Arc<dyn Fn(typst::syntax::Span, u16) -> usize + Send + Sync + 'static>,
    >,
    hit_map_collector: &mut HitMap,
) {
    let mut font_family = text_item.font.info().family.to_string();
    let weight = text_item.font.info().variant.weight.to_number();

    if font_family == "New Computer Modern Math" {
        font_family = "NewComputerModernMath".to_string();
    }

    let font_id = if let Some(id) = resolve_font_with_fallback(&font_family, weight as u16, cx) {
        id
    } else {
        cx.text_system()
            .resolve_font(&gpui::font(font_family.clone()))
    };

    let font_size = Pixels::from(text_item.size.to_pt() as f32 * scale_factor);
    let mut x_cursor = Pixels::ZERO;

    let total_width_pt: f32 = text_item
        .glyphs
        .iter()
        .map(|g| g.x_advance.at(text_item.size).to_pt() as f32)
        .sum();
    let text_size_pt = text_item.size.to_pt() as f32;

    for glyph_instance in &text_item.glyphs {
        let text_color = match &text_item.fill {
            typst::visualize::Paint::Gradient(gradient) => {
                let x = x_cursor.as_f32() / scale_factor;
                let color = gradient.sample_at((x, 0.0), (total_width_pt, text_size_pt));
                typst_color_to_gpui_hsla(&color)
            }
            _ => typst_paint_to_gpui_hsla_from_paint(&text_item.fill),
        };

        let glyph_id: GlyphId = unsafe { std::mem::transmute(glyph_instance.id as u32) };
        let x_offset = glyph_instance.x_offset.at(text_item.size).to_pt() as f32;
        let y_offset = glyph_instance.y_offset.at(text_item.size).to_pt() as f32;

        let glyph_origin = item_absolute_origin_gpui
            + gpui::point(
                x_cursor + Pixels::from(x_offset * scale_factor),
                Pixels::from(y_offset * scale_factor),
            );

        window
            .paint_glyph(glyph_origin, font_id, glyph_id, font_size, text_color)
            .unwrap();

        let glyph_width =
            Pixels::from(glyph_instance.x_advance.at(text_item.size).to_pt() as f32 * scale_factor);
        let glyph_height = font_size;

        let (span, index) = glyph_instance.span;
        let glyph_range = glyph_instance.range();

        let byte_offset = if let Some(resolver) = span_resolver {
            resolver(span, index)
        } else {
            glyph_range.start
        };

        hit_map_collector.push_glyph(GlyphInfo {
            bounds: Bounds::new(glyph_origin, size(glyph_width, glyph_height)),
            byte_offset,
            byte_len: glyph_range.len(),
            span,
        });

        x_cursor +=
            Pixels::from(glyph_instance.x_advance.at(text_item.size).to_pt() as f32 * scale_factor);
    }
}
