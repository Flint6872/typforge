mod frame_item;
mod frame_item_text;
mod typst_curve;
mod typst_point;
mod utils;

use typst_curve::TypstCurveExt;
use typst_point::TypstPointExt;

use crate::PreviewPanelEvent;
use gpui::{
    App, Bounds, Element, ElementId, EventEmitter, LayoutId, Pixels, Point, TransformationMatrix,
    Window,
};
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
use typst::text::Font;
use typst::{
    layout::{Frame, FrameItem},
    syntax::Span,
};
use typst_layout::PagedDocument;

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
                gpui::TransformationMatrix::unit(),
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
        current_transform: TransformationMatrix,
        hit_map_collector: &mut HitMap,
    ) {
        for (item_relative_pos_typst, frame_item_variant) in frame.items() {
            // `item_absolute_origin_gpui` here represents the UNTRANSFORMED base origin of the current item,
            // relative to the `origin` of the current frame/group.
            // The `current_transform` will be applied to the *contents* of this item.
            let item_absolute_origin_gpui =
                origin + item_relative_pos_typst.to_gpui_pixels(scale_factor);

            let item_document_y =
                y_offset_from_top + item_relative_pos_typst.to_gpui_pixels(scale_factor).y;

            match frame_item_variant {
                FrameItem::Text(text_item) => {
                    frame_item_text::frame_item_text(
                        text_item,
                        item_absolute_origin_gpui,
                        scale_factor,
                        window,
                        cx,
                        &self.span_resolver,
                        current_transform,
                        hit_map_collector,
                    );
                }

                FrameItem::Image(image, typst_image_size, _span) => {
                    frame_item::frame_item_image(
                        image,
                        typst_image_size,
                        item_absolute_origin_gpui,
                        scale_factor,
                        window,
                        cx,
                        current_transform,
                        &self.render_state,
                    );
                }

                FrameItem::Group(group_item) => {
                    // 1. Convert the Typst group transform to GPUI matrix
                    let group_local_transform =
                        utils::typst_transform_to_gpui_matrix(group_item.transform, scale_factor);

                    // 2. Compose: ParentTransform * GroupTransform
                    let new_current_transform = current_transform.compose(group_local_transform);

                    self.paint_frame_items(
                        item_absolute_origin_gpui, // This is already the transformed origin
                        y_offset_from_top,
                        scale_factor,
                        &group_item.frame,
                        window,
                        cx,
                        depth + 1,
                        new_current_transform, // Pass the NEW composed transform
                        hit_map_collector,
                    );
                }

                FrameItem::Shape(shape, _span) => {
                    frame_item::frame_item_shape(
                        self,
                        shape,
                        item_absolute_origin_gpui,
                        scale_factor,
                        window,
                        cx,
                        y_offset_from_top,
                        current_transform,
                        hit_map_collector,
                    );
                }

                FrameItem::Link(dest, size) => {
                    frame_item::frame_item_link(
                        dest,
                        *size,
                        item_absolute_origin_gpui,
                        scale_factor,
                        current_transform,
                        hit_map_collector,
                    );
                }
                FrameItem::Tag(tag) => {
                    frame_item::frame_item_tag(tag, item_document_y, hit_map_collector);
                }
            }
        }
    }

    pub fn render_tiling(
        &self, // Added &self here
        window: &mut Window,
        cx: &mut App,
        tiling: &typst::visualize::Tiling,
        bounds: gpui::Bounds<Pixels>,
        y_offset_from_top: Pixels,
        scale_factor: f32,
        current_transform: TransformationMatrix,
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
                    current_transform,
                    hit_map,
                );
            }
        }
    }
}
