use crate::typst_element::{
    GlyphInfo, HitMap,
    utils::{
        resolve_font_with_fallback, typst_color_to_gpui_hsla, typst_paint_to_gpui_hsla_from_paint,
    },
};
use gpui::{App, Bounds, GlyphId, PathBuilder, Pixels, Point, TransformationMatrix, Window, size};
use typst::text::TextItem;

// Adapter to pipe TrueType glyph outlines directly into GPUI's PathBuilder
struct GlyphPathBuilder {
    builder: PathBuilder,
    scale_x: f32,
    scale_y: f32,
}

impl ttf_parser::OutlineBuilder for GlyphPathBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.builder.move_to(gpui::point(
            gpui::px(x * self.scale_x),
            gpui::px(y * self.scale_y),
        ));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.builder.line_to(gpui::point(
            gpui::px(x * self.scale_x),
            gpui::px(y * self.scale_y),
        ));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.builder.curve_to(
            gpui::point(gpui::px(x * self.scale_x), gpui::px(y * self.scale_y)),
            gpui::point(gpui::px(x1 * self.scale_x), gpui::px(y1 * self.scale_y)),
        );
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.builder.cubic_bezier_to(
            gpui::point(gpui::px(x * self.scale_x), gpui::px(y * self.scale_y)),
            gpui::point(gpui::px(x1 * self.scale_x), gpui::px(y1 * self.scale_y)),
            gpui::point(gpui::px(x2 * self.scale_x), gpui::px(y2 * self.scale_y)),
        );
    }

    fn close(&mut self) {
        self.builder.close();
    }
}

pub fn frame_item_text(
    text_item: &TextItem,
    item_absolute_origin_gpui: Point<Pixels>,
    scale_factor: f32,
    window: &mut Window,
    cx: &mut App,
    span_resolver: &Option<
        std::sync::Arc<dyn Fn(typst::syntax::Span, u16) -> usize + Send + Sync + 'static>,
    >,
    current_transform: TransformationMatrix,
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

    // Typst's Font exposes the underlying ttf_parser::Face reference
    let face = text_item.font.ttf();
    let units_per_em = face.units_per_em() as f32;

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

        // 1. Calculate local glyph origin (baseline position within the text block)
        let local_x = x_cursor
            + Pixels::from(
                glyph_instance.x_offset.at(text_item.size).to_pt() as f32 * scale_factor,
            );
        let local_y =
            Pixels::from(glyph_instance.y_offset.at(text_item.size).to_pt() as f32 * scale_factor);

        let final_glyph_origin = item_absolute_origin_gpui
            + gpui::point(
                gpui::px(
                    current_transform.rotation_scale[0][0] * local_x.as_f32()
                        + current_transform.rotation_scale[0][1] * local_y.as_f32()
                        + current_transform.translation[0],
                ),
                gpui::px(
                    current_transform.rotation_scale[1][0] * local_x.as_f32()
                        + current_transform.rotation_scale[1][1] * local_y.as_f32()
                        + current_transform.translation[1],
                ),
            );

        // 2. Fetch the TrueType outline for vector painting if a transform is active
        if current_transform != TransformationMatrix::unit() {
            let ttf_glyph_id = ttf_parser::GlyphId(glyph_instance.id);
            let mut glyph_path_builder = GlyphPathBuilder {
                builder: PathBuilder::fill(),
                // Flip the Y-axis (TrueType is Y-up, GPUI is Y-down)
                scale_x: font_size.as_f32() / units_per_em,
                scale_y: -font_size.as_f32() / units_per_em,
            };

            if face
                .outline_glyph(ttf_glyph_id, &mut glyph_path_builder)
                .is_some()
            {
                // 1. Create a transform containing ONLY the rotation and scale
                // This rotates and skews the glyph around its local (0,0) baseline
                let lyon_transform = gpui::Transform::new(
                    current_transform.rotation_scale[0][0],
                    -current_transform.rotation_scale[0][1],
                    -current_transform.rotation_scale[1][0],
                    current_transform.rotation_scale[1][1],
                    0.0, // Set translation to 0 here
                    0.0, // Set translation to 0 here
                );
                glyph_path_builder.builder.transform(lyon_transform);

                // 2. Position the rotated/scaled glyph at its final screen coordinate.
                // Using .translate() here will append the translation AFTER the rotation.
                glyph_path_builder.builder.translate(final_glyph_origin);

                if let Ok(path) = glyph_path_builder.builder.build() {
                    window.paint_path(path, gpui::solid_background(text_color));
                }
            }
        } else {
            // Fallback to standard high-performance raster paint_glyph when no rotation is active
            window
                .paint_glyph(final_glyph_origin, font_id, glyph_id, font_size, text_color)
                .unwrap();
        }

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
            bounds: Bounds::new(final_glyph_origin, size(glyph_width, glyph_height)),
            byte_offset,
            byte_len: glyph_range.len(),
            span,
        });

        x_cursor +=
            Pixels::from(glyph_instance.x_advance.at(text_item.size).to_pt() as f32 * scale_factor);
    }
}
