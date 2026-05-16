use gpui::{App, Pixels};

pub fn typst_paint_to_gpui_hsla_from_paint(paint: &typst::visualize::Paint) -> gpui::Hsla {
    match paint {
        typst::visualize::Paint::Solid(color) => typst_color_to_gpui_hsla(color),
        typst::visualize::Paint::Gradient(gradient) => {
            // Sample the center for a single color representation
            let color = gradient.sample_at((0.5, 0.5), (1.0, 1.0)); // (x,y), (width,height)
            typst_color_to_gpui_hsla(&color)
        }
        _ => gpui::black(), // Fallback for unsupported paint types
    }
}

pub fn typst_color_to_gpui_hsla(color: &typst::visualize::Color) -> gpui::Hsla {
    let rgb = color.to_rgb();
    gpui::Rgba {
        r: rgb.red,
        g: rgb.green,
        b: rgb.blue,
        a: rgb.alpha,
    }
    .into()
}

pub fn typst_paint_to_gpui_background(paint: &typst::visualize::Paint) -> gpui::Background {
    // Use TypstPaint alias
    match paint {
        typst::visualize::Paint::Solid(color) => {
            gpui::solid_background(typst_color_to_gpui_hsla(color))
        }
        typst::visualize::Paint::Gradient(gradient_arc) => {
            match gradient_arc {
                typst::visualize::Gradient::Linear(linear) => {
                    let stops: Vec<gpui::LinearColorStop> = linear
                        .stops
                        .iter()
                        .map(|(color, offset)| gpui::LinearColorStop {
                            color: typst_color_to_gpui_hsla(color),
                            percentage: offset.get() as f32,
                        })
                        .collect();

                    if stops.is_empty() {
                        return gpui::solid_background(gpui::transparent_black());
                    }

                    // Angle Fix: +90.0 to align Typst and GPUI coordinate systems
                    let angle = (linear.angle.to_deg() as f32 + 90.0) % 360.0;

                    if stops.len() < 2 {
                        let color = stops
                            .first()
                            .map(|s| s.color)
                            .unwrap_or(gpui::transparent_black());
                        return gpui::solid_background(color);
                    }

                    // LIMITATION: GPUI 0.2.2 only supports 2 stops per quad.
                    // This is why complex gradients (rainbow, repeat) only work on
                    // Rects (which we tessellate manually) and not Circles/Lines yet.
                    gpui::linear_gradient(angle, stops[0].clone(), stops[stops.len() - 1].clone())
                }
                typst::visualize::Gradient::Radial(_radial) => {
                    // GPUI 0.2.2 has no native Radial Gradient Background.
                    // For now, we sample the middle of the gradient to provide a solid color fallback.
                    let color = gradient_arc.sample_at((0.5, 0.5), (1.0, 1.0));
                    gpui::solid_background(typst_color_to_gpui_hsla(&color))
                }
                typst::visualize::Gradient::Conic(_conic) => {
                    // GPUI 0.2.2 has no native Conic Gradient Background.
                    // Fallback to solid color.
                    let color = gradient_arc.sample_at((0.5, 0.5), (1.0, 1.0));
                    gpui::solid_background(typst_color_to_gpui_hsla(&color))
                }
            }
        }

        // Handle Tiling as a separate Paint variant
        typst::visualize::Paint::Tiling(_) => {
            // This `typst_paint_to_gpui_background` function is generally for returning
            // a single `gpui::Background` for `paint_quad`. For Tiling, the actual rendering
            // happens in `render_tiling` which is called earlier.
            // Here, we return a transparent background as a placeholder.
            gpui::solid_background(gpui::transparent_black())
        }
    }
}

pub fn resolve_font_with_fallback(family: &str, weight: u16, cx: &App) -> Option<gpui::FontId> {
    let text_system = cx.text_system();

    // 1. Try exact name
    let mut request = gpui::font(family.to_string());
    request.weight = gpui::FontWeight(weight as f32);
    let id = text_system.resolve_font(&request);
    if let Some(resolved) = text_system.get_font_for_id(id) {
        if resolved.family == family {
            return Some(id);
        }
    }

    // 2. Try name without spaces (e.g. NewComputerModernMath)
    let collapsed = family.replace(" ", "");
    if collapsed != family {
        let mut request = gpui::font(collapsed.clone());
        request.weight = gpui::FontWeight(weight as f32);
        let id = text_system.resolve_font(&request);
        if let Some(resolved) = text_system.get_font_for_id(id) {
            if resolved.family == collapsed {
                return Some(id);
            }
        }
    }

    None
}

// Helper for converting Typst's LineCap to GPUI's StrokeCap
// Helper for converting Typst's LineCap to lyon_path::LineCap
pub fn typst_linecap_to_gpui(cap: &typst::visualize::LineCap) -> lyon_path::LineCap {
    match cap {
        typst::visualize::LineCap::Butt => lyon_path::LineCap::Butt,
        typst::visualize::LineCap::Round => lyon_path::LineCap::Round,
        typst::visualize::LineCap::Square => lyon_path::LineCap::Square,
    }
}

// Helper for converting Typst's LineJoin to lyon_path::LineJoin
pub fn typst_linejoin_to_gpui(join: &typst::visualize::LineJoin) -> lyon_path::LineJoin {
    match join {
        typst::visualize::LineJoin::Miter => lyon_path::LineJoin::Miter,
        typst::visualize::LineJoin::Round => lyon_path::LineJoin::Round,
        typst::visualize::LineJoin::Bevel => lyon_path::LineJoin::Bevel,
    }
}

// Helper for converting Typst's DashPattern to GPUI's dash array and offset
pub fn typst_dash_to_gpui(
    // Now takes the exact type from FixedStroke::dash
    dash_pattern_option: &Option<
        typst::visualize::DashPattern<typst::layout::Abs, typst::layout::Abs>,
    >,
    scale_factor: f32,
) -> (Option<Vec<Pixels>>, Pixels) {
    if let Some(dash) = dash_pattern_option {
        // Since `dash.array` is `Vec<Abs>`, `l` here is `&Abs`.
        // We can just `map` it directly to Pixels.
        let dash_array: Vec<Pixels> = dash
            .array
            .iter()
            .map(|&length_abs| {
                // Dereference `length_abs` as it's `&Abs`
                Pixels::from(length_abs.to_pt() as f32 * scale_factor)
            })
            .collect();

        let dash_offset = Pixels::from(dash.phase.to_pt() as f32 * scale_factor);

        (Some(dash_array), dash_offset)
    } else {
        (None, Pixels::ZERO)
    }
}
