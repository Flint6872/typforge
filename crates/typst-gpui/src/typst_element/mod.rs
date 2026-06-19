mod typst_curve;
mod typst_point;
mod utils;

use typst_curve::TypstCurveExt;
use typst_layout::PagedDocument;
use typst_point::TypstPointExt;
use utils::{
    resolve_font_with_fallback, typst_color_to_gpui_hsla, typst_dash_to_gpui,
    typst_paint_to_gpui_background, typst_paint_to_gpui_hsla_from_paint,
};

use gpui::{
    App, Bounds, Element, ElementId, EventEmitter, GlyphId, LayoutId, Pixels, Point, Window,
};

use crate::PreviewPanelEvent;
use parking_lot::Mutex;
use std::{
    collections::HashMap,
    fmt::Debug,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
use typst::{
    layout::{Frame, FrameItem},
    syntax::Span,
    visualize::{Gradient, Paint},
};

const DPI: f32 = 96.0;
const PT_TO_PX: f32 = DPI / 72.0;

// NEW: Struct to store information about each rendered glyph for hit-testing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphInfo {
    pub bounds: Bounds<Pixels>, // Bounding box of the glyph in screen pixels
    pub byte_offset: usize,
    pub byte_len: usize, // UTF-8 byte offset in the source file
    pub span: Span,      // Typst's source span for this glyph
}

#[derive(Debug, Clone)]
pub struct AnimationState {
    current_frame_index: usize,
    last_frame_updated_time: Instant,
}

#[derive(Clone)]
pub struct LinkInfo {
    pub bounds: Bounds<Pixels>,
    pub destination: typst::model::Destination,
}

#[derive(Clone)]
pub struct AnchorInfo {
    pub location: typst::introspection::Location,
    pub position: Point<Pixels>,
}

#[derive(Default, Clone)]
pub struct HitMap {
    pub glyphs: Vec<GlyphInfo>,
    pub links: Vec<LinkInfo>,
    pub anchors: Vec<AnchorInfo>,
}

impl HitMap {
    pub fn push_glyph(&mut self, info: GlyphInfo) {
        self.glyphs.push(info);
    }

    pub fn push_link(&mut self, bounds: Bounds<Pixels>, destination: typst::model::Destination) {
        self.links.push(LinkInfo {
            bounds,
            destination,
        });
    }

    pub fn push_anchor(
        &mut self,
        location: typst::introspection::Location,
        position: Point<Pixels>,
    ) {
        self.anchors.push(AnchorInfo { location, position });
    }
}

pub struct TypstRenderState {
    pub image_cache: Mutex<HashMap<typst::visualize::Image, Arc<gpui::Image>>>,
    pub animation_cache: Mutex<HashMap<gpui::ImageId, AnimationState>>,
    pub has_active_animations: AtomicBool,
}

impl Default for TypstRenderState {
    fn default() -> Self {
        Self {
            image_cache: Mutex::new(HashMap::new()),
            animation_cache: Mutex::new(HashMap::new()),
            has_active_animations: AtomicBool::new(false),
        }
    }
}

// Our custom GPUI element for rendering Typst content.
pub struct TypstElement {
    id: ElementId,
    document: Arc<PagedDocument>, // This will hold the compiled Typst document.
    page_margin: f32,
    zoom: f32,

    render_state: Arc<TypstRenderState>,
    //scroll_offset: Point<Pixels>,
    cursor_offset: Option<usize>,
    selection_range: Option<std::ops::Range<usize>>,
    on_hit_map_updated: Option<Arc<Mutex<dyn FnMut(HitMap, &mut App) + Send + Sync + 'static>>>,
    show_cursor: bool,
    pub span_resolver: Option<Arc<dyn Fn(Span, u16) -> usize + Send + Sync + 'static>>,
}

// Manual implementation of Debug for TypstElement
impl Debug for TypstElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypstElement")
            .field("id", &self.id)
            .field("document", &Arc::as_ptr(&self.document)) // Ptr for debugging Arc
            .field("page_margin", &self.page_margin)
            .field("zoom", &self.zoom)
            .field("cursor_offset", &self.cursor_offset)
            // DO NOT include `on_hit_map_updated` here, as it doesn't implement Debug
            .finish()
    }
}

// Manual implementation of PartialEq
impl PartialEq for TypstElement {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && // Include ID in equality check
            Arc::<PagedDocument>::ptr_eq(&self.document, &other.document)
                && self.page_margin == other.page_margin
                && self.zoom == other.zoom
    }
}

impl gpui::IntoElement for TypstElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
    // `into_any_element` will use the default implementation now that TypstElement derives Debug.
}

impl Element for TypstElement {
    type RequestLayoutState = (); // No complex layout needed for the root element itself
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    // Request layout: For the root element, we just need to indicate our size.
    // The actual layout is done by Typst internally. We'll pass the document's size.
    fn request_layout(
        &mut self,
        _id: Option<&gpui::GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let scale = PT_TO_PX * self.zoom;
        let mut total_height = 0.0;
        let mut max_width: f32 = 0.0;

        for (i, page) in self.document.pages().iter().enumerate() {
            let size = page.frame.size();
            total_height += size.y.to_pt() as f32 * scale;
            max_width = max_width.max(size.x.to_pt() as f32 * scale);

            if i < self.document.pages().len() - 1 {
                // Scale the margin as well
                total_height += self.page_margin * self.zoom;
            }
        }

        let layout_id = window.request_layout(
            gpui::Style {
                size: gpui::Size {
                    width: gpui::px(max_width).into(),
                    height: gpui::px(total_height).into(),
                },
                ..Default::default()
            },
            [],
            cx,
        );

        (layout_id, ())
    }

    // Prepaint: Prepare the frame items for painting.
    fn prepaint(
        &mut self,
        _id: Option<&gpui::GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        _bounds: gpui::Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    // Paint: Draw the actual content.
    fn paint(
        &mut self,
        _id: Option<&gpui::GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: gpui::Bounds<Pixels>, // Bounds allocated by request_layout
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let scale_factor = PT_TO_PX * self.zoom;
        let mut generated_hit_map: HitMap = HitMap::default();
        let page_margin_px = gpui::px(self.page_margin * self.zoom);

        let mut current_page_screen_y = bounds.origin.y; // For actual painting
        let mut y_offset_from_top = Pixels::ZERO; // For STABLE hit-mapping

        for (i, page) in self.document.pages().iter().enumerate() {
            let page_height = Pixels::from(page.frame.height().to_pt() as f32 * scale_factor);
            let frame_origin_in_gpui = gpui::point(bounds.origin.x, current_page_screen_y);

            let page_width = Pixels::from(page.frame.width().to_pt() as f32 * scale_factor);

            let page_size_gpui = gpui::Size {
                width: page_width,
                height: page_height,
            };
            // Draw Background
            window.paint_quad(gpui::quad(
                gpui::Bounds::new(frame_origin_in_gpui, page_size_gpui),
                gpui::Corners::default(),
                gpui::rgb(0xFFFFFF),
                gpui::Edges::default(),
                gpui::black(),
                gpui::BorderStyle::default(),
            ));

            self.render_state
                .has_active_animations
                .store(false, Ordering::Relaxed);
            // Paint items (Passing page.frame directly - NO CLONE)
            self.paint_frame_items(
                frame_origin_in_gpui,
                y_offset_from_top, // <--- New stable argument
                scale_factor,
                &page.frame,
                window,
                cx,
                1,
                &mut generated_hit_map,
            );

            // Advance both counters by exactly the same amount
            let advance = page_height
                + if i < self.document.pages().len() - 1 {
                    page_margin_px
                } else {
                    Pixels::ZERO
                };

            current_page_screen_y += advance;
            y_offset_from_top += advance;
        }
        // Immutable borrow of self.document.pages ends here.

        if let Some(callback_mutex) = &self.on_hit_map_updated {
            // Lock the mutex to get mutable access to the FnMut closure
            let mut locked_callback = callback_mutex.lock();
            locked_callback(generated_hit_map.clone(), cx); // Now you can call it
        }

        // --- Draw Selection Highlight ---
        if let Some(selection) = &self.selection_range {
            let sel_start = selection.start.min(selection.end);
            let sel_end = selection.start.max(selection.end);

            if sel_start != sel_end {
                let highlight_color = gpui::Rgba {
                    r: 0.29,
                    g: 0.56,
                    b: 0.88,
                    a: 0.3,
                };

                for glyph_info in &generated_hit_map.glyphs {
                    let glyph_end = glyph_info.byte_offset + glyph_info.byte_len;
                    // Check for any overlap between selection and glyph
                    if glyph_end > sel_start && glyph_info.byte_offset < sel_end {
                        let mut highlight_bounds = glyph_info.bounds;
                        highlight_bounds.origin.y -= highlight_bounds.size.height * 0.8;
                        window.paint_quad(gpui::fill(highlight_bounds, highlight_color));
                    }
                }
            }
        }

        // --- Draw Cursor ---
        if let Some(cursor_offset) = self.cursor_offset {
            let mut cursor_visual_position_px: Option<gpui::Point<Pixels>> = None;
            let mut cursor_line_height_px: Option<Pixels> = None;

            for glyph_info in &generated_hit_map.glyphs {
                // Find glyph containing the cursor
                if cursor_offset >= glyph_info.byte_offset
                    && cursor_offset < glyph_info.byte_offset + glyph_info.byte_len
                {
                    cursor_visual_position_px = Some(glyph_info.bounds.origin);
                    cursor_line_height_px = Some(glyph_info.bounds.size.height);
                    break;
                }
            }

            // End of document/line handling
            if cursor_visual_position_px.is_none() && !generated_hit_map.glyphs.is_empty() {
                let last_glyph = generated_hit_map.glyphs.last().unwrap();
                if cursor_offset >= last_glyph.byte_offset + last_glyph.byte_len {
                    cursor_visual_position_px = Some(gpui::point(
                        last_glyph.bounds.top_right().x,
                        last_glyph.bounds.origin.y,
                    ));
                    cursor_line_height_px = Some(last_glyph.bounds.size.height);
                }
            }

            if let Some(mut point_px) = cursor_visual_position_px {
                let cursor_height = cursor_line_height_px.unwrap_or(gpui::px(16.0));

                // Shift up to baseline
                point_px.y -= cursor_height * 0.8;

                let cursor_rect = gpui::Bounds {
                    origin: point_px, // USE point_px DIRECTLY (No bounds.origin addition!)
                    size: gpui::Size {
                        width: gpui::px(1.5),
                        height: cursor_height,
                    },
                };

                if self.show_cursor {
                    window.paint_quad(gpui::quad(
                        cursor_rect,
                        gpui::Corners::default(),
                        gpui::rgb(0x4a90e2),
                        gpui::Edges::default(),
                        gpui::black(),
                        gpui::BorderStyle::default(),
                    ));
                }
            }
        }
    }
}

impl EventEmitter<PreviewPanelEvent> for TypstElement {}

impl TypstElement {
    pub fn new(
        document: Arc<PagedDocument>,
        render_state: Arc<TypstRenderState>,
        //scroll_offset: Point<Pixels>,
        cursor_offset: Option<usize>,
        selection_range: Option<std::ops::Range<usize>>,
        on_hit_map_updated: Option<Arc<Mutex<dyn FnMut(HitMap, &mut App) + Send + Sync + 'static>>>,
        show_cursor: bool,
        span_resolver: Option<Arc<dyn Fn(Span, u16) -> usize + Send + Sync + 'static>>, // Add this
    ) -> Self {
        Self {
            id: gpui::ElementId::from(0),
            document,
            page_margin: 20.0,
            zoom: 1.0,

            render_state,
            //scroll_offset,
            cursor_offset,
            selection_range,
            on_hit_map_updated,
            show_cursor,
            span_resolver,
        }
    }

    pub fn with_zoom(mut self, zoom: f32) -> Self {
        self.zoom = zoom;
        self
    }

    fn paint_frame_items(
        &self,
        origin: Point<Pixels>,
        y_offset_from_top: Pixels,
        scale_factor: f32,
        frame: &Frame,
        window: &mut Window,
        cx: &mut App,
        depth: usize,
        hit_map_collector: &mut HitMap,
    ) {
        for (item_relative_pos_typst, frame_item_variant) in frame.items() {
            let item_absolute_origin_gpui =
                origin + item_relative_pos_typst.to_gpui_pixels(scale_factor);

            let item_relative_pos_gpui = item_relative_pos_typst.to_gpui_pixels(scale_factor);

            // This is for drawing pixels on the screen
            //let item_screen_origin = origin + item_relative_pos_gpui;

            // This is for the stable scroll target (Top of Page + Item Offset)
            let item_document_y = y_offset_from_top + item_relative_pos_gpui.y;

            match frame_item_variant {
                FrameItem::Text(text_item) => {
                    let mut font_family = text_item.font.info().family.to_string();
                    let weight = text_item.font.info().variant.weight.to_number();

                    // 1. Precise Mapping for New Computer Modern
                    // We match exactly what your logs showed GPUI registered.
                    if font_family == "New Computer Modern Math" {
                        font_family = "NewComputerModernMath".to_string();
                    }
                    // eprintln!(
                    //     "DEBUG: Typst requested font family: '{}' (ID: {}, weight: {})",
                    //     font_family,
                    //     text_item.font.index(),
                    //     weight
                    // );

                    // 2. Resolve with specific weight/style to prevent GPUI from falling back
                    let mut font_request = gpui::font(font_family.clone());
                    font_request.weight = gpui::FontWeight(weight as f32);

                    // Try original name, then try name without spaces as a fallback
                    let font_id =
                        if let Some(id) = resolve_font_with_fallback(&font_family, weight, cx) {
                            id
                        } else {
                            // If both fail, let GPUI fall back to system default
                            cx.text_system()
                                .resolve_font(&gpui::font(font_family.clone()))
                        };

                    // 3. DEBUG: Verify if GPUI actually found the font
                    if let Some(resolved) = cx.text_system().get_font_for_id(font_id) {
                        if resolved.family != font_family {
                            println!("!!! FALLBACK DETECTED !!!");
                            println!("Typst wanted: '{}'", font_family);
                            println!("GPUI used:   '{}'", resolved.family);
                        }
                    }

                    let font_size = Pixels::from(text_item.size.to_pt() as f32 * scale_factor);
                    let mut x_cursor = Pixels::ZERO;

                    // Calculate metrics for gradient sampling relative to the text run
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
                                let color = gradient.sample_at(
                                    (x, 0.0), // Sample at baseline
                                    (total_width_pt, text_size_pt),
                                );
                                typst_color_to_gpui_hsla(&color)
                            }
                            _ => typst_paint_to_gpui_hsla_from_paint(&text_item.fill),
                        };

                        let glyph_id: GlyphId =
                            unsafe { std::mem::transmute(glyph_instance.id as u32) };

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

                        let glyph_width = Pixels::from(
                            glyph_instance.x_advance.at(text_item.size).to_pt() as f32
                                * scale_factor,
                        );
                        let glyph_height = font_size;

                        let (span, index) = glyph_instance.span;
                        let glyph_range = glyph_instance.range();

                        // --- NEW: Map relative span offsets to absolute document offsets ---
                        let byte_offset = if let Some(resolver) = &self.span_resolver {
                            resolver(span, index)
                        } else {
                            glyph_range.start
                        };

                        hit_map_collector.push_glyph(GlyphInfo {
                            bounds: Bounds::new(
                                glyph_origin,
                                gpui::size(glyph_width, glyph_height),
                            ),
                            byte_offset, // Uses the resolved absolute offset!
                            byte_len: glyph_range.len(),
                            span,
                        });

                        let x_advance = glyph_instance.x_advance.at(text_item.size).to_pt() as f32;
                        x_cursor += Pixels::from(x_advance * scale_factor);
                    }
                }

                FrameItem::Image(image, typst_image_size, _span) => {
                    let width_px = Pixels::from(typst_image_size.x.to_pt() as f32 * scale_factor);
                    let height_px = Pixels::from(typst_image_size.y.to_pt() as f32 * scale_factor);
                    let image_bounds = gpui::Bounds::new(
                        item_absolute_origin_gpui,
                        gpui::size(width_px, height_px),
                    );

                    // 1. Get or Create the high-level GPUI Image asset (cached)
                    let gpui_image_arc = {
                        let mut cache = self.render_state.image_cache.lock();
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
                            let mut animation_cache = self.render_state.animation_cache.lock();
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
                            let elapsed_since_last_update = current_paint_time
                                .duration_since(animation_state.last_frame_updated_time);

                            // --- NEW DEBUGGING BLOCK ---
                            let _is_first_entry =
                                animation_state.last_frame_updated_time == current_paint_time; // Check if it's the very first time

                            // --- END NEW DEBUGGING BLOCK ---

                            if elapsed_since_last_update >= frame_delay_duration {
                                animation_state.current_frame_index =
                                    (animation_state.current_frame_index + 1)
                                        % render_image.frame_count();
                                animation_state.last_frame_updated_time = current_paint_time;
                            }
                            current_frame_index = animation_state.current_frame_index;

                            self.render_state
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
                        // Image is still loading or failed.
                        // When use_render_image returns None, it implies the asset is not ready yet.
                        // To trigger a re-render when it *is* ready, GPUI usually handles this via its
                        // asset system's internal notification. However, if that's not happening,
                        // we'd need to manually notify the view associated with this element.
                        // For TypstElement, its ID is used.
                        // This might already be happening if gpui::Image::use_render_image uses `cx.notify()`.
                        // If not, we'd need to explicitly `cx.notify(self.id());`
                        // For now, let's just draw the fallback.
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

                FrameItem::Group(group_item) => {
                    self.paint_frame_items(
                        item_absolute_origin_gpui,
                        y_offset_from_top,
                        scale_factor,
                        &group_item.frame,
                        window,
                        cx,
                        depth + 1,
                        hit_map_collector,
                    );
                }

                FrameItem::Shape(shape, _span) => {
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
                            // For curves, use bbox_size for tiling if it's a closed shape
                            typst::visualize::Geometry::Curve(curve) if curve.is_closed() => {
                                // **New: Check for closed curve**
                                let typst_bbox_size = curve.bbox(None).size();
                                let w =
                                    Pixels::from(typst_bbox_size.x.to_pt() as f32 * scale_factor);
                                let h =
                                    Pixels::from(typst_bbox_size.y.to_pt() as f32 * scale_factor);
                                gpui::Bounds::new(item_absolute_origin_gpui, gpui::size(w, h))
                            }
                            _ => gpui::Bounds::new(
                                item_absolute_origin_gpui,
                                gpui::size(Pixels::ZERO, Pixels::ZERO),
                            ),
                        };

                        if bbox.size.width > Pixels::ZERO && bbox.size.height > Pixels::ZERO {
                            self.render_tiling(
                                window,
                                cx,
                                tiling_paint,
                                bbox,
                                y_offset_from_top,
                                scale_factor,
                                hit_map_collector,
                            );
                            was_tiling_applied = true;
                        }
                    }

                    match &shape.geometry {
                        typst::visualize::Geometry::Rect(size) => {
                            let w = Pixels::from(size.x.to_pt() as f32 * scale_factor);
                            let h = Pixels::from(size.y.to_pt() as f32 * scale_factor);
                            let bounds =
                                gpui::Bounds::new(item_absolute_origin_gpui, gpui::size(w, h));

                            let mut was_gradient_tessellated = false;
                            if let Some(Paint::Gradient(grad)) = &shape.fill {
                                if let Gradient::Linear(linear) = grad {
                                    let angle_deg = linear.angle.to_deg() as f32;

                                    if (angle_deg % 180.0).abs() < 1.0
                                        || (angle_deg % 180.0 - 90.0).abs() < 1.0
                                    {
                                        let is_vertical = (angle_deg % 180.0 - 90.0).abs() < 1.0;
                                        was_gradient_tessellated = true;

                                        for stops in linear.stops.windows(2) {
                                            let (c1, p1) = (&stops[0].0, stops[0].1.get() as f32);
                                            let (c2, p2) = (&stops[1].0, stops[1].1.get() as f32);

                                            let sub_origin = if is_vertical {
                                                item_absolute_origin_gpui
                                                    + gpui::point(Pixels::ZERO, h * p1)
                                            } else {
                                                item_absolute_origin_gpui
                                                    + gpui::point(w * p1, Pixels::ZERO)
                                            };

                                            let sub_size = if is_vertical {
                                                gpui::size(w, h * (p2 - p1))
                                            } else {
                                                gpui::size(w * (p2 - p1), h)
                                            };

                                            window.paint_quad(gpui::quad(
                                                gpui::Bounds::new(sub_origin, sub_size),
                                                gpui::Corners::default(),
                                                gpui::linear_gradient(
                                                    (angle_deg + 90.0) % 360.0,
                                                    gpui::LinearColorStop {
                                                        color: typst_color_to_gpui_hsla(c1),
                                                        percentage: 0.0,
                                                    },
                                                    gpui::LinearColorStop {
                                                        color: typst_color_to_gpui_hsla(c2),
                                                        percentage: 1.0,
                                                    },
                                                ),
                                                gpui::Edges::all(thickness),
                                                stroke_color,
                                                gpui::BorderStyle::default(),
                                            ));
                                        }
                                    }
                                }
                            }

                            if !was_tiling_applied && !was_gradient_tessellated {
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
                                let line_thickness_px = Pixels::from(
                                    typst_stroke.thickness.to_pt() as f32 * scale_factor,
                                );

                                // Check if we should tessellate the line for a multi-stop gradient
                                let mut was_tessellated = false;
                                if let Paint::Gradient(grad) = &typst_stroke.paint {
                                    if let Gradient::Linear(linear) = grad {
                                        if linear.stops.len() > 2 {
                                            was_tessellated = true;
                                            for stops in linear.stops.windows(2) {
                                                let (c1, p1) =
                                                    (&stops[0].0, stops[0].1.get() as f32);
                                                let (c2, p2) =
                                                    (&stops[1].0, stops[1].1.get() as f32);

                                                // Interpolate start/end points for this segment
                                                let sub_start = start_p + target_gpui_rel * p1;
                                                let sub_end = start_p + target_gpui_rel * p2;

                                                let mut path_builder =
                                                    gpui::PathBuilder::stroke(line_thickness_px);
                                                path_builder.move_to(sub_start);
                                                path_builder.line_to(sub_end);

                                                if let Ok(tessellated_path) = path_builder.build() {
                                                    // Correct angle for line direction
                                                    let angle = (linear.angle.to_deg() as f32
                                                        + 90.0)
                                                        % 360.0;
                                                    let sub_bg = gpui::linear_gradient(
                                                        angle,
                                                        gpui::LinearColorStop {
                                                            color: typst_color_to_gpui_hsla(c1),
                                                            percentage: 0.0,
                                                        },
                                                        gpui::LinearColorStop {
                                                            color: typst_color_to_gpui_hsla(c2),
                                                            percentage: 1.0,
                                                        },
                                                    );
                                                    window.paint_path(tessellated_path, sub_bg);
                                                }
                                            }
                                        }
                                    }
                                }

                                // Fallback: Single segment for simple colors/gradients
                                if !was_tessellated {
                                    let mut path_builder =
                                        gpui::PathBuilder::stroke(line_thickness_px);
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
                        }

                        typst::visualize::Geometry::Curve(curve) => {
                            let typst_bbox_size = curve.bbox(None).size();
                            let w = Pixels::from(typst_bbox_size.x.to_pt() as f32 * scale_factor);
                            let h = Pixels::from(typst_bbox_size.y.to_pt() as f32 * scale_factor);
                            let bounds =
                                gpui::Bounds::new(item_absolute_origin_gpui, gpui::size(w, h));

                            let has_fill = shape.fill.is_some();
                            let has_stroke = shape
                                .stroke
                                .as_ref()
                                .is_some_and(|s| s.thickness.to_pt() > 0.0);

                            let is_ellipse = curve.is_ellipse();
                            let is_circle = is_ellipse && (w.as_f32() - h.as_f32()).abs() < 0.1;

                            // 1. High-quality Circle/Ellipse rendering via paint_quad
                            // This handles circles, squares, and "pill" shapes.
                            if (is_circle || is_ellipse) && !was_tiling_applied {
                                let corner_radius = if is_circle {
                                    w / 2.0
                                } else {
                                    // For general ellipses, we use the smaller dimension's half
                                    // to create a "pill" shape (best GPUI approximation).
                                    w.min(h) / 2.0
                                };

                                window.paint_quad(gpui::quad(
                                    bounds,
                                    gpui::Corners::all(corner_radius),
                                    fill_background,
                                    gpui::Edges::all(thickness),
                                    stroke_color,
                                    gpui::BorderStyle::default(),
                                ));
                            } else {
                                // 2. Generic Curve / Polygon Fallback

                                // Render fill as a background quad (approximation)
                                if has_fill
                                    && !was_tiling_applied
                                    && w > Pixels::ZERO
                                    && h > Pixels::ZERO
                                {
                                    window.paint_quad(gpui::quad(
                                        bounds,
                                        gpui::Corners::default(),
                                        fill_background,
                                        gpui::Edges::all(Pixels::ZERO),
                                        gpui::transparent_black(),
                                        gpui::BorderStyle::default(),
                                    ));
                                }

                                // Render stroke using an accurate path
                                if has_stroke
                                    && !was_tiling_applied
                                    && (w > Pixels::ZERO || h > Pixels::ZERO)
                                {
                                    let mut gpui_path = gpui::Path::new(item_absolute_origin_gpui);
                                    let mut last_p = typst::layout::Point::zero();
                                    let mut first_p = None;

                                    for item in curve.0.iter() {
                                        match item {
                                            typst::visualize::CurveItem::Move(p) => {
                                                gpui_path.move_to(
                                                    item_absolute_origin_gpui
                                                        + p.to_gpui_pixels(scale_factor),
                                                );
                                                last_p = *p;
                                                if first_p.is_none() {
                                                    first_p = Some(*p);
                                                }
                                            }
                                            typst::visualize::CurveItem::Line(p) => {
                                                gpui_path.line_to(
                                                    item_absolute_origin_gpui
                                                        + p.to_gpui_pixels(scale_factor),
                                                );
                                                last_p = *p;
                                            }
                                            typst::visualize::CurveItem::Cubic(c1, c2, p) => {
                                                // We must flatten cubics, otherwise they look like squares!
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
                                                        item_absolute_origin_gpui
                                                            + pt.to_gpui_pixels(scale_factor),
                                                    );
                                                }
                                                last_p = *p;
                                            }
                                            typst::visualize::CurveItem::Close => {
                                                if let Some(p) = first_p {
                                                    gpui_path.line_to(
                                                        item_absolute_origin_gpui
                                                            + p.to_gpui_pixels(scale_factor),
                                                    );
                                                }
                                            }
                                        }
                                    }
                                    window.paint_path(gpui_path, stroke_color);
                                }
                            }
                        } //_ => {}
                    }
                }

                FrameItem::Link(destination, size) => {
                    let width = Pixels::from(size.x.to_pt() as f32 * scale_factor);
                    let height = Pixels::from(size.y.to_pt() as f32 * scale_factor);

                    let bounds =
                        gpui::Bounds::new(item_absolute_origin_gpui, gpui::size(width, height));

                    hit_map_collector.push_link(bounds, destination.clone());
                }

                FrameItem::Tag(tag) => {
                    let location = tag.location();
                    // We store the stable Y coordinate
                    hit_map_collector
                        .push_anchor(location, gpui::point(Pixels::ZERO, item_document_y));
                }
            }
        }
    }

    fn render_tiling(
        &self, // Added &self here
        window: &mut Window,
        cx: &mut App,
        tiling: &typst::visualize::Tiling,
        bounds: gpui::Bounds<Pixels>,
        y_offset_from_top: Pixels,
        scale_factor: f32,
        hit_map: &mut HitMap,
    ) {
        let cell_w = Pixels::from(tiling.size().x.to_pt() as f32 * scale_factor);
        let cell_h = Pixels::from(tiling.size().y.to_pt() as f32 * scale_factor);

        if cell_w <= Pixels::ZERO || cell_h <= Pixels::ZERO {
            return;
        }

        // Corrected: Use f32::from() for explicit conversion to f32 before division
        let cols = (f32::from(bounds.size.width) / f32::from(cell_w)).ceil() as i32;
        let rows = (f32::from(bounds.size.height) / f32::from(cell_h)).ceil() as i32;

        for row in 0..rows {
            for col in 0..cols {
                let offset = gpui::point(cell_w * col as f32, cell_h * row as f32);
                let sub_origin = bounds.origin + offset;

                self.paint_frame_items(
                    sub_origin,
                    y_offset_from_top,
                    scale_factor,
                    tiling.frame(),
                    window,
                    cx,
                    100, // Arbitrary depth limit for recursion
                    hit_map,
                );
            }
        }
    }
}
