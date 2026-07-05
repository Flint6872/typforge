// C:\Users\xerxe\Rust\typforge0.0.1\crates\typst-gpui\src\typst_element\frame_item.rs

use crate::typst_element::{
    AnimationState, GradientCacheKey, HitMap, TypstCurveExt, TypstElement, TypstPointExt,
    utils::{
        typst_color_to_gpui_hsla, typst_dash_to_gpui, typst_paint_to_gpui_background,
        typst_paint_to_gpui_hsla_from_paint,
    },
};
use gpui::{App, Bounds, PathBuilder, Pixels, Point, TransformationMatrix, Window};
use std::{sync::Arc, time::Instant};
use typst::{
    foundations::Smart,
    layout::Size,
    visualize::{Gradient, Image, Paint},
};

// --- Helper Functions for SVG Masks & CPU Rasterization ---

// Helper to calculate the bounding box for a line, accounting for stroke thickness
fn calculate_line_bbox(
    origin_gpui: Point<Pixels>,
    target_rel_typst: typst::layout::Point, // This is the relative end point in Typst units
    stroke_thickness_pt: f32,               // Line's stroke thickness in Typst points
    scale_factor: f32,
) -> gpui::Bounds<Pixels> {
    let target_gpui_rel = target_rel_typst.to_gpui_pixels(scale_factor);
    let end_p = origin_gpui + target_gpui_rel;

    let min_x = origin_gpui.x.min(end_p.x);
    let max_x = origin_gpui.x.max(end_p.x);
    let min_y = origin_gpui.y.min(end_p.y);
    let max_y = origin_gpui.y.max(end_p.y);

    let half_stroke_px = Pixels::from(stroke_thickness_pt / 2.0 * scale_factor);

    gpui::Bounds::new(
        gpui::point(min_x - half_stroke_px, min_y - half_stroke_px),
        gpui::size(
            max_x - min_x + half_stroke_px * 2.0,
            max_y - min_y + half_stroke_px * 2.0,
        ),
    )
}

// Helper to generate a monochrome SVG mask of a shape geometry
fn render_shape_mask_as_svg(
    geometry: &typst::visualize::Geometry,
    stroke: Option<&typst::visualize::FixedStroke>,
    fill_rule: typst::visualize::FillRule, // <-- ADDED fill_rule parameter
    scale_factor: f32,
) -> Option<Vec<u8>> {
    let (w, h) = match geometry {
        typst::visualize::Geometry::Rect(size) => (
            size.x.to_pt() as f32 * scale_factor,
            size.y.to_pt() as f32 * scale_factor,
        ),
        typst::visualize::Geometry::Curve(curve) => {
            let bbox_size = curve.bbox(None).size();
            (
                bbox_size.x.to_pt() as f32 * scale_factor,
                bbox_size.y.to_pt() as f32 * scale_factor,
            )
        }
        _ => return None,
    };

    if w <= 0.0 || h <= 0.0 {
        return None;
    }

    // Calculate padding needed to prevent stroke clipping
    let thickness = stroke
        .map(|s| s.thickness.to_pt() as f32 * scale_factor)
        .unwrap_or(0.0);
    let padding = thickness;
    let half_padding = padding / 2.0;

    let svg_w = w + padding;
    let svg_h = h + padding;

    let mut svg = String::new();
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}">"#,
        svg_w, svg_h, svg_w, svg_h
    ));

    let fill_attr = if stroke.is_none() {
        r#"fill="black""#
    } else {
        r#"fill="none""#
    };

    let stroke_attr = if let Some(stroke) = stroke {
        let mut attr = format!(r#"stroke="black" stroke-width="{}""#, thickness);

        // --- ADD THESE LINES TO ENSURE CAPS AND JOINS ARE ADDED ---
        let line_cap = match stroke.cap {
            typst::visualize::LineCap::Butt => "butt",
            typst::visualize::LineCap::Round => "round",
            typst::visualize::LineCap::Square => "square",
        };
        let line_join = match stroke.join {
            typst::visualize::LineJoin::Miter => "miter",
            typst::visualize::LineJoin::Bevel => "bevel",
            typst::visualize::LineJoin::Round => "round",
        };
        attr.push_str(&format!(
            r#" stroke-linecap="{}" stroke-linejoin="{}""#,
            line_cap, line_join
        ));
        // --- END ADDITION ---

        if let Some(dash) = &stroke.dash {
            let dash_array: Vec<String> = dash
                .array
                .iter()
                .map(|len| (len.to_pt() as f32 * scale_factor).to_string())
                .collect();
            attr.push_str(&format!(
                r#" stroke-dasharray="{}" stroke-dashoffset="{}""#,
                dash_array.join(","),
                dash.phase.to_pt() as f32 * scale_factor
            ));
        }
        attr
    } else {
        r#"stroke="none""#.to_string()
    };

    let svg_fill_rule_attr = match fill_rule {
        // Convert Typst fill rule to SVG attribute
        typst::visualize::FillRule::NonZero => r#"fill-rule="nonzero""#,
        typst::visualize::FillRule::EvenOdd => r#"fill-rule="evenodd""#,
    };

    match geometry {
        typst::visualize::Geometry::Rect(_) => {
            svg.push_str(&format!(
                r#"<rect x="{}" y="{}" width="{}" height="{}" {} {} />"#,
                half_padding, half_padding, w, h, fill_attr, stroke_attr
            ));
        }
        typst::visualize::Geometry::Curve(curve) => {
            if curve.is_ellipse() {
                let rx = w / 2.0;
                let ry = h / 2.0;
                svg.push_str(&format!(
                    r#"<ellipse cx="{}" cy="{}" rx="{}" ry="{}" {} {} />"#,
                    rx + half_padding,
                    ry + half_padding,
                    rx,
                    ry,
                    fill_attr,
                    stroke_attr
                ));
            } else {
                svg.push_str(&format!(
                    r#"<g transform="translate({}, {})">"#,
                    half_padding, half_padding
                ));
                let mut d = String::new();
                for item in curve.0.iter() {
                    match item {
                        typst::visualize::CurveItem::Move(p) => {
                            d.push_str(&format!(
                                "M {} {} ",
                                p.x.to_pt() as f32 * scale_factor,
                                p.y.to_pt() as f32 * scale_factor
                            ));
                        }
                        typst::visualize::CurveItem::Line(p) => {
                            d.push_str(&format!(
                                "L {} {} ",
                                p.x.to_pt() as f32 * scale_factor,
                                p.y.to_pt() as f32 * scale_factor
                            ));
                        }
                        typst::visualize::CurveItem::Cubic(c1, c2, p) => {
                            d.push_str(&format!(
                                "C {} {}, {} {}, {} {} ",
                                c1.x.to_pt() as f32 * scale_factor,
                                c1.y.to_pt() as f32 * scale_factor,
                                c2.x.to_pt() as f32 * scale_factor,
                                c2.y.to_pt() as f32 * scale_factor,
                                p.x.to_pt() as f32 * scale_factor,
                                p.y.to_pt() as f32 * scale_factor
                            ));
                        }
                        typst::visualize::CurveItem::Close => {
                            d.push_str("Z ");
                        }
                    }
                }
                // <-- HERE: Add the fill-rule to the path tag
                svg.push_str(&format!(
                    r#"<path d="{}" {} {} {} />"#,
                    d, svg_fill_rule_attr, fill_attr, stroke_attr
                ));
                svg.push_str("</g>");
            }
        }
        _ => return None,
    }

    svg.push_str("</svg>");
    Some(svg.into_bytes())
}

// Helper to rasterize Typst Gradients to raw PNG bytes
fn rasterize_gradient_to_png(
    gradient: &typst::visualize::Gradient,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    let w_f = width as f32;
    let h_f = height as f32;
    for y in 0..height {
        for x in 0..width {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let color = gradient.sample_at((px, py), (w_f, h_f));
            let rgb = color.to_rgb();
            pixels.push((rgb.red * 255.0).round() as u8);
            pixels.push((rgb.green * 255.0).round() as u8);
            pixels.push((rgb.blue * 255.0).round() as u8);
            pixels.push((rgb.alpha * 255.0).round() as u8);
        }
    }

    let mut png_bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        if let Ok(mut writer) = encoder.write_header() {
            let _ = writer.write_image_data(&pixels);
        }
    }
    png_bytes
}

// We will pass a reference to the TypstElement or a struct containing these dependencies
pub fn frame_item_image(
    image: &Image,
    typst_image_size: &Size,
    origin: Point<Pixels>,
    scale_factor: f32,
    window: &mut Window,
    cx: &mut App,
    _current_transform: TransformationMatrix, // Transformation support limited
    render_state: &Arc<crate::typst_element::TypstRenderState>,
) {
    let width_px = Pixels::from(typst_image_size.x.to_pt() as f32 * scale_factor);
    let height_px = Pixels::from(typst_image_size.y.to_pt() as f32 * scale_factor);
    let image_bounds = gpui::Bounds::new(origin, gpui::size(width_px, height_px));

    // Get or Create the high-level GPUI Image asset (cached)
    let gpui_image_arc = {
        let mut cache = render_state.image_cache.lock();
        cache.entry(image.clone()).or_insert_with(|| {
            match image.kind() {
                typst::visualize::ImageKind::Raster(raster_image) => {
                    let format = match raster_image.format() {
                        typst::visualize::RasterFormat::Exchange(typst::visualize::ExchangeFormat::Png) => Some(gpui::ImageFormat::Png),
                        typst::visualize::RasterFormat::Exchange(typst::visualize::ExchangeFormat::Jpg) => Some(gpui::ImageFormat::Jpeg),
                        typst::visualize::RasterFormat::Exchange(typst::visualize::ExchangeFormat::Gif) => Some(gpui::ImageFormat::Gif),
                        typst::visualize::RasterFormat::Exchange(typst::visualize::ExchangeFormat::Webp) => Some(gpui::ImageFormat::Webp),
                        _ => None,
                    };
                    format.map(|gpui_format| Arc::new(gpui::Image::from_bytes(gpui_format, raster_image.data().to_vec())))
                }
                typst::visualize::ImageKind::Svg(svg_image) => {
                    Some(Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Svg, svg_image.data().to_vec())))
                }
                typst::visualize::ImageKind::Pdf(_pdf_image) => {
                    eprintln!("Warning: PDF images are not directly rendered here. Rasterization required.");
                    None
                }
            }
            .unwrap_or_else(|| Arc::new(gpui::Image::empty())) // Store an empty image if decoding failed
        }).clone()
    };

    // Request the RenderImage (GPU texture) and Paint it
    if let Some(render_image) = gpui_image_arc.use_render_image(window, cx) {
        let mut current_frame_index = 0; // Default to first frame

        // --- Animation Logic ---
        if render_image.frame_count() > 1 {
            let mut animation_cache = render_state.animation_cache.lock();
            let image_id_for_cache = render_image.id;
            let current_paint_time = Instant::now();

            let animation_state = animation_cache
                .entry(image_id_for_cache)
                .or_insert_with(|| AnimationState {
                    current_frame_index: 0,
                    last_frame_updated_time: current_paint_time,
                });

            let frame_delay_duration: std::time::Duration = render_image
                .delay(animation_state.current_frame_index)
                .into();

            let elapsed_since_last_update =
                current_paint_time.duration_since(animation_state.last_frame_updated_time);

            if elapsed_since_last_update >= frame_delay_duration {
                animation_state.current_frame_index =
                    (animation_state.current_frame_index + 1) % render_image.frame_count();
                animation_state.last_frame_updated_time = current_paint_time;
            }
            current_frame_index = animation_state.current_frame_index;

            render_state
                .has_active_animations
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }

        window
            .paint_image(
                image_bounds,
                gpui::Corners::default(),
                render_image,
                current_frame_index,
                false, // grayscale
            )
            .ok();
    } else {
        window.paint_quad(gpui::quad(
            image_bounds,
            gpui::Corners::default(),
            gpui::black().alpha(0.05),
            gpui::Edges::all(Pixels::from(1.0)),
            gpui::black().alpha(0.1),
            gpui::BorderStyle::default(),
        ));
    }
}

pub fn frame_item_link(
    destination: &typst::model::Destination,
    size: typst::layout::Size,
    origin: Point<Pixels>,
    scale_factor: f32,
    current_transform: TransformationMatrix,
    hit_map: &mut HitMap,
) {
    let width = Pixels::from(size.x.to_pt() as f32 * scale_factor);
    let height = Pixels::from(size.y.to_pt() as f32 * scale_factor);

    let transformed_origin = current_transform.apply(origin);

    let bounds = Bounds::new(transformed_origin, gpui::size(width, height));
    hit_map.push_link(bounds, destination.clone());
}

pub fn frame_item_tag(
    tag: &typst::introspection::Tag,
    item_document_y: Pixels,
    hit_map: &mut HitMap,
) {
    hit_map.push_anchor(tag.location(), Point::new(Pixels::ZERO, item_document_y));
}

pub fn frame_item_shape(
    element: &TypstElement,
    shape: &typst::visualize::Shape,
    item_absolute_origin_gpui: Point<Pixels>,
    scale_factor: f32,
    window: &mut Window,
    cx: &mut App,
    y_offset_from_top: Pixels,
    _current_transform: TransformationMatrix,
    hit_map_collector: &mut HitMap,
) {
    // --- Initial setup for stroke and fill properties ---
    let stroke = shape.stroke.as_ref();
    let stroke_color = stroke
        .map(|s| typst_paint_to_gpui_hsla_from_paint(&s.paint))
        .unwrap_or(gpui::transparent_black());
    let thickness = stroke
        .map(|s| Pixels::from(s.thickness.to_pt() as f32 * scale_factor))
        .unwrap_or(Pixels::ZERO);

    // --- Handle Tiling/Patterns (self-contained logic) ---
    if let Some(Paint::Tiling(tiling_paint)) = &shape.fill {
        let bbox = match &shape.geometry {
            typst::visualize::Geometry::Rect(size) => {
                let w = Pixels::from(size.x.to_pt() as f32 * scale_factor);
                let h = Pixels::from(size.y.to_pt() as f32 * scale_factor);
                gpui::Bounds::new(item_absolute_origin_gpui, gpui::size(w, h))
            }
            typst::visualize::Geometry::Curve(curve) if curve.is_closed() => {
                let typst_bbox_size = curve.bbox(None).size();
                let w = Pixels::from(typst_bbox_size.x.to_pt() as f32 * scale_factor);
                let h = Pixels::from(typst_bbox_size.y.to_pt() as f32 * scale_factor);
                gpui::Bounds::new(item_absolute_origin_gpui, gpui::size(w, h))
            }
            // For Tiling shapes with Geometry::Line:
            typst::visualize::Geometry::Line(target) => {
                let line_stroke_thickness_pt = shape
                    .stroke
                    .as_ref()
                    .map(|s| s.thickness.to_pt() as f32)
                    .unwrap_or(1.0); // Default to 1pt for bbox if line stroke isn't explicitly set.

                calculate_line_bbox(
                    item_absolute_origin_gpui,
                    *target, // Pass the Typst point
                    line_stroke_thickness_pt,
                    scale_factor,
                )
            }
            _ => gpui::Bounds::new(
                item_absolute_origin_gpui,
                gpui::size(Pixels::ZERO, Pixels::ZERO),
            ),
        };

        if bbox.size.width > Pixels::ZERO && bbox.size.height > Pixels::ZERO {
            // Render the tiled fill
            element.render_tiling(
                window,
                cx,
                tiling_paint,
                bbox, // Use the correctly calculated bbox for the outer shape
                y_offset_from_top,
                scale_factor,
                gpui::TransformationMatrix::unit(),
                hit_map_collector,
            );

            // Render the stroke of the outer shape (if any)
            if thickness > Pixels::ZERO {
                let corner_radius = match &shape.geometry {
                    typst::visualize::Geometry::Rect(_) => gpui::Corners::default(),
                    typst::visualize::Geometry::Curve(curve) if curve.is_ellipse() => {
                        let w_bbox =
                            Pixels::from(curve.bbox(None).size().x.to_pt() as f32 * scale_factor);
                        let h_bbox =
                            Pixels::from(curve.bbox(None).size().y.to_pt() as f32 * scale_factor);
                        if curve.is_circle(w_bbox, h_bbox) {
                            gpui::Corners::all(w_bbox / 2.0)
                        } else {
                            gpui::Corners::all(w_bbox.min(h_bbox) / 2.0)
                        }
                    }
                    _ => gpui::Corners::default(),
                };

                window.paint_quad(gpui::quad(
                    bbox,
                    corner_radius,
                    gpui::solid_background(gpui::transparent_black()),
                    gpui::Edges::all(thickness),
                    stroke_color,
                    gpui::BorderStyle::default(),
                ));
            }
            return;
        }
    }

    // --- Determine bounding box for general rendering (outside tiling block) ---
    // This block is for non-tiled shapes.
    let bbox = match &shape.geometry {
        typst::visualize::Geometry::Rect(size) => {
            let w = Pixels::from(size.x.to_pt() as f32 * scale_factor);
            let h = Pixels::from(size.y.to_pt() as f32 * scale_factor);
            gpui::Bounds::new(item_absolute_origin_gpui, gpui::size(w, h))
        }
        typst::visualize::Geometry::Curve(curve) => {
            let typst_bbox_size = curve.bbox(None).size();
            let w = Pixels::from(typst_bbox_size.x.to_pt() as f32 * scale_factor);
            let h = Pixels::from(typst_bbox_size.y.to_pt() as f32 * scale_factor);
            gpui::Bounds::new(item_absolute_origin_gpui, gpui::size(w, h))
        }
        typst::visualize::Geometry::Line(target) => {
            // <--- NEW ARM
            let line_stroke_thickness_pt = shape
                .stroke
                .as_ref()
                .map(|s| s.thickness.to_pt() as f32)
                .unwrap_or(1.0);

            calculate_line_bbox(
                item_absolute_origin_gpui,
                *target,
                line_stroke_thickness_pt,
                scale_factor,
            )
        }
        _ => gpui::Bounds::new(
            item_absolute_origin_gpui,
            gpui::size(Pixels::ZERO, Pixels::ZERO),
        ),
    };

    // Only proceed if the bounding box has a valid size
    if bbox.size.width <= Pixels::ZERO || bbox.size.height <= Pixels::ZERO {
        return;
    }

    // ... (rest of the gradient and SVG mask rendering logic) ...
    // This part remains mostly the same, as the initial bbox calculation handles lines now.

    // --- High-Fidelity Gradient Fill (Rasterized PNG) ---
    if let Some(Paint::Gradient(grad)) = &shape.fill {
        let width_px = bbox.size.width.as_f32().round() as u32;
        let height_px = bbox.size.height.as_f32().round() as u32;

        if width_px > 0 && height_px > 0 {
            let cache_key = GradientCacheKey {
                gradient: grad.clone(),
                width: width_px,
                height: height_px,
            };

            // This is where gpui_image_arc is defined.
            let gpui_image_arc = {
                let mut cache = element.render_state.gradient_cache.lock();
                cache
                    .entry(cache_key)
                    .or_insert_with(|| {
                        let png_bytes = rasterize_gradient_to_png(grad, width_px, height_px);
                        Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png, png_bytes))
                    })
                    .clone()
            };

            // Move the usage of gpui_image_arc INSIDE this block
            if let Some(render_image) = gpui_image_arc.use_render_image(window, cx) {
                let corner_radius =
                    if let typst::visualize::Geometry::Curve(curve) = &shape.geometry {
                        if curve.is_ellipse() {
                            if curve.is_circle(bbox.size.width, bbox.size.height) {
                                gpui::Corners::all(bbox.size.width / 2.0)
                            } else {
                                gpui::Corners::all(bbox.size.width.min(bbox.size.height) / 2.0)
                            }
                        } else {
                            gpui::Corners::default()
                        }
                    } else {
                        gpui::Corners::default()
                    };

                window
                    .paint_image(
                        bbox,
                        corner_radius,
                        render_image,
                        0,     // frame_index
                        false, // grayscale
                    )
                    .ok();

                // Render stroke on top of the gradient fill if one exists
                if thickness > Pixels::ZERO {
                    window.paint_quad(gpui::quad(
                        bbox,
                        corner_radius,
                        gpui::solid_background(gpui::transparent_black()), // Transparent fill
                        gpui::Edges::all(thickness),
                        stroke_color,
                        gpui::BorderStyle::default(),
                    ));
                }
                return;
            }
        }
    }

    // --- High-Quality SVG Mask Rendering for Curves/Rects with Solid Colors and Strokes ---
    let should_render_via_svg_mask =
        matches!(&shape.geometry, typst::visualize::Geometry::Curve(_))
            || matches!(&shape.geometry, typst::visualize::Geometry::Rect(_));

    if should_render_via_svg_mask {
        let stroke_thickness = thickness.as_f32();
        let half_thickness = stroke_thickness / 2.0;

        let padded_bbox = gpui::Bounds::new(
            bbox.origin - gpui::point(Pixels::from(half_thickness), Pixels::from(half_thickness)),
            gpui::size(
                bbox.size.width + Pixels::from(stroke_thickness),
                bbox.size.height + Pixels::from(stroke_thickness),
            ),
        );

        if let Some(Paint::Solid(solid_color)) = &shape.fill {
            if let Some(svg_bytes) =
                render_shape_mask_as_svg(&shape.geometry, None, shape.fill_rule, scale_factor)
            {
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                svg_bytes.hash(&mut hasher);
                let path_hash = format!("shape-fill-svg-{}", hasher.finish());

                window
                    .paint_svg(
                        padded_bbox,
                        path_hash.into(),
                        Some(&svg_bytes),
                        gpui::TransformationMatrix::unit(),
                        typst_color_to_gpui_hsla(solid_color),
                        cx,
                    )
                    .ok();
            }
        }

        if let Some(stroke) = &shape.stroke {
            if stroke.thickness.to_pt() > 0.0 {
                if let Some(svg_bytes) = render_shape_mask_as_svg(
                    &shape.geometry,
                    Some(stroke),
                    shape.fill_rule,
                    scale_factor,
                ) {
                    use std::hash::{Hash, Hasher};
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    svg_bytes.hash(&mut hasher);
                    let path_hash = format!("shape-stroke-svg-{}", hasher.finish());

                    window
                        .paint_svg(
                            padded_bbox,
                            path_hash.into(),
                            Some(&svg_bytes),
                            gpui::TransformationMatrix::unit(),
                            stroke_color,
                            cx,
                        )
                        .ok();
                }
            }
        }
        return;
    }

    // --- Final Fallback Rendering for Simple Lines (if not handled by earlier SVG/Gradient paths) ---
    match &shape.geometry {
        typst::visualize::Geometry::Line(target) => {
            let target_gpui_rel = target.to_gpui_pixels(scale_factor);
            let start_p = item_absolute_origin_gpui;
            let end_p = item_absolute_origin_gpui + target_gpui_rel;

            if let Some(typst_stroke) = shape.stroke.as_ref() {
                let line_thickness_px =
                    Pixels::from(typst_stroke.thickness.to_pt() as f32 * scale_factor);

                let mut path_builder = gpui::PathBuilder::stroke(line_thickness_px);
                path_builder.move_to(start_p);
                path_builder.line_to(end_p);

                let (dash_array, _dash_offset) =
                    typst_dash_to_gpui(&typst_stroke.dash, scale_factor);
                if let Some(da) = dash_array {
                    path_builder = path_builder.dash_array(&da);
                }

                if let Ok(tessellated_path) = path_builder.build() {
                    window.paint_path(
                        tessellated_path,
                        typst_paint_to_gpui_background(&typst_stroke.paint),
                    );
                }
            }
        }
        _ => { /* Other geometries are covered by early returns or not supported for direct fallback */
        }
    }
}
