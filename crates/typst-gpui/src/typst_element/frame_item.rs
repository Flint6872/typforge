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
    layout::Size,
    visualize::{Gradient, Image, Paint},
};

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

    // 1. Get or Create the high-level GPUI Image asset (cached)
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

    // 2. Request the RenderImage (GPU texture) and Paint it
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

            // Calculate how much time has truly passed since this frame was last displayed
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
                .store(true, std::sync::atomic::Ordering::Relaxed); // Use shared flag
        }
        // --- End Animation Logic ---

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
    let fill_background = shape
        .fill
        .as_ref()
        .map(typst_paint_to_gpui_background)
        .unwrap_or_else(|| gpui::solid_background(gpui::transparent_black()));

    let stroke = shape.stroke.as_ref();
    let stroke_color = stroke
        .map(|s| typst_paint_to_gpui_hsla_from_paint(&s.paint))
        .unwrap_or(gpui::transparent_black());
    let thickness = stroke
        .map(|s| Pixels::from(s.thickness.to_pt() as f32 * scale_factor))
        .unwrap_or(Pixels::ZERO);

    // --- Handle Tiling/Patterns - always attempt to render if present ---
    let mut was_tiling_applied = false;
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
            _ => gpui::Bounds::new(
                item_absolute_origin_gpui,
                gpui::size(Pixels::ZERO, Pixels::ZERO),
            ),
        };

        if bbox.size.width > Pixels::ZERO && bbox.size.height > Pixels::ZERO {
            element.render_tiling(
                window,
                cx,
                tiling_paint,
                bbox,
                y_offset_from_top,
                scale_factor,
                gpui::TransformationMatrix::unit(),
                hit_map_collector,
            );
            was_tiling_applied = true;
        }
    }

    if !was_tiling_applied {
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
            _ => gpui::Bounds::new(
                item_absolute_origin_gpui,
                gpui::size(Pixels::ZERO, Pixels::ZERO),
            ),
        };

        if bbox.size.width > Pixels::ZERO && bbox.size.height > Pixels::ZERO {
            // High-fidelity CPU rasterization of ALL Typst Gradients (Linear, Radial, Conic)
            if let Some(Paint::Gradient(grad)) = &shape.fill {
                let width_px = bbox.size.width.as_f32().round() as u32;
                let height_px = bbox.size.height.as_f32().round() as u32;

                if width_px > 0 && height_px > 0 {
                    let cache_key = GradientCacheKey {
                        gradient: grad.clone(),
                        width: width_px,
                        height: height_px,
                    };

                    let gpui_image_arc = {
                        let mut cache = element.render_state.gradient_cache.lock();
                        cache
                            .entry(cache_key)
                            .or_insert_with(|| {
                                let png_bytes =
                                    rasterize_gradient_to_png(grad, width_px, height_px);
                                Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png, png_bytes))
                            })
                            .clone()
                    };

                    if let Some(render_image) = gpui_image_arc.use_render_image(window, cx) {
                        let corner_radius = if matches!(&shape.geometry, typst::visualize::Geometry::Curve(curve) if curve.is_ellipse())
                        {
                            bbox.size.width.min(bbox.size.height) / 2.0
                        } else {
                            Pixels::ZERO
                        };

                        // Render the gradient fill
                        window
                            .paint_image(
                                bbox,
                                gpui::Corners::all(corner_radius),
                                render_image,
                                0,     // frame_index
                                false, // grayscale
                            )
                            .ok();

                        // Render the stroke on top (if one exists)
                        if thickness > Pixels::ZERO {
                            window.paint_quad(gpui::quad(
                                bbox,
                                gpui::Corners::all(corner_radius),
                                gpui::solid_background(gpui::transparent_black()),
                                gpui::Edges::all(thickness),
                                stroke_color,
                                gpui::BorderStyle::default(),
                            ));
                        }

                        return; // Successfully rendered gradient shape!
                    }
                }
            }
        }
    }

    match &shape.geometry {
        typst::visualize::Geometry::Rect(size) => {
            let w = Pixels::from(size.x.to_pt() as f32 * scale_factor);
            let h = Pixels::from(size.y.to_pt() as f32 * scale_factor);
            let bounds = gpui::Bounds::new(item_absolute_origin_gpui, gpui::size(w, h));

            if !was_tiling_applied {
                window.paint_quad(gpui::quad(
                    bounds,
                    gpui::Corners::default(),
                    fill_background,
                    gpui::Edges::all(thickness),
                    stroke_color,
                    gpui::BorderStyle::default(),
                ));
            }
        }
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

        typst::visualize::Geometry::Curve(curve) => {
            let typst_bbox_size = curve.bbox(None).size();
            let w = Pixels::from(typst_bbox_size.x.to_pt() as f32 * scale_factor);
            let h = Pixels::from(typst_bbox_size.y.to_pt() as f32 * scale_factor);
            let bounds = gpui::Bounds::new(item_absolute_origin_gpui, gpui::size(w, h));

            let has_fill = shape.fill.is_some();
            let has_stroke = shape
                .stroke
                .as_ref()
                .is_some_and(|s| s.thickness.to_pt() > 0.0);

            let is_ellipse = curve.is_ellipse();
            let is_circle = is_ellipse && (w.as_f32() - h.as_f32()).abs() < 0.1;

            if (is_circle || is_ellipse) && !was_tiling_applied {
                let corner_radius = if is_circle { w / 2.0 } else { w.min(h) / 2.0 };

                window.paint_quad(gpui::quad(
                    bounds,
                    gpui::Corners::all(corner_radius),
                    fill_background,
                    gpui::Edges::all(thickness),
                    stroke_color,
                    gpui::BorderStyle::default(),
                ));
            } else {
                if has_fill && !was_tiling_applied && w > Pixels::ZERO && h > Pixels::ZERO {
                    window.paint_quad(gpui::quad(
                        bounds,
                        gpui::Corners::default(),
                        fill_background,
                        gpui::Edges::all(Pixels::ZERO),
                        gpui::transparent_black(),
                        gpui::BorderStyle::default(),
                    ));
                }

                if has_stroke && !was_tiling_applied && (w > Pixels::ZERO || h > Pixels::ZERO) {
                    let mut gpui_path = gpui::Path::new(item_absolute_origin_gpui);
                    let mut last_p = typst::layout::Point::zero();
                    let mut first_p = None;

                    for item in curve.0.iter() {
                        match item {
                            typst::visualize::CurveItem::Move(p) => {
                                gpui_path.move_to(
                                    item_absolute_origin_gpui + p.to_gpui_pixels(scale_factor),
                                );
                                last_p = *p;
                                if first_p.is_none() {
                                    first_p = Some(*p);
                                }
                            }
                            typst::visualize::CurveItem::Line(p) => {
                                gpui_path.line_to(
                                    item_absolute_origin_gpui + p.to_gpui_pixels(scale_factor),
                                );
                                last_p = *p;
                            }
                            typst::visualize::CurveItem::Cubic(c1, c2, p) => {
                                const SEGMENTS: usize = 12;
                                for i in 1..=SEGMENTS {
                                    let t = i as f32 / SEGMENTS as f32;
                                    let inv_t = 1.0 - t;
                                    let b0 = inv_t * inv_t * inv_t;
                                    let b1 = 3.0 * inv_t * inv_t * t;
                                    let b2 = 3.0 * inv_t * t * t;
                                    let b3 = t * t * t;
                                    let pt = typst::layout::Point::new(
                                        last_p.x * b0.into()
                                            + c1.x * b1.into()
                                            + c2.x * b2.into()
                                            + p.x * b3.into(),
                                        last_p.y * b0.into()
                                            + c1.y * b1.into()
                                            + c2.y * b2.into()
                                            + p.y * b3.into(),
                                    );
                                    gpui_path.line_to(
                                        item_absolute_origin_gpui + pt.to_gpui_pixels(scale_factor),
                                    );
                                }
                                last_p = *p;
                            }
                            typst::visualize::CurveItem::Close => {
                                if let Some(p) = first_p {
                                    gpui_path.line_to(
                                        item_absolute_origin_gpui + p.to_gpui_pixels(scale_factor),
                                    );
                                }
                            }
                        }
                    }
                    window.paint_path(gpui_path, stroke_color);
                }
            }
        }
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
