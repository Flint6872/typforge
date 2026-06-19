//typforge0.0.1\crates\typst-gpui\src\lib.rs
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    input::{Input, InputEvent, InputState, RopeExt},
    scroll::ScrollableElement,
};
use parking_lot::Mutex;
use std::{sync::Arc, time::Duration};
use typst_layout::PagedDocument;

pub mod typst_element;
use crate::typst_element::{HitMap, TypstElement, TypstRenderState};

/// Trait defining what the PreviewPanel needs from a Typst World.
pub trait TypstGpuiWorld: typst::World + Send + Sync + 'static {
    fn set_source(&mut self, source: String);
    fn set_main_document_info(&mut self, path: Option<std::path::PathBuf>, content: String);

    fn document(&self) -> Option<std::sync::Arc<PagedDocument>> {
        None
    }
    fn set_document(&mut self, _doc: std::sync::Arc<PagedDocument>) {}
}

#[derive(Debug, PartialEq, Eq)]
pub enum PreviewPanelEvent {
    SourceChanged(String),
    DiagnosticsChanged(Vec<typst::diag::SourceDiagnostic>),
    SelectionChanged(std::ops::Range<usize>), // <-- Added SelectionChanged event
}

/// The PreviewPanel is a GPUI View that renders a Typst document.
pub struct PreviewPanel<W: TypstGpuiWorld> {
    world: Arc<Mutex<W>>,
    document: Option<std::sync::Arc<PagedDocument>>,
    pub render_state: Arc<TypstRenderState>,
    diagnostics: Vec<typst::diag::SourceDiagnostic>,
    focus_handle: FocusHandle,
    zoom: f32,
    pub input_state: Entity<InputState>,
    _input_state_subscription: Option<Subscription>,
    pub suppressing_events: bool,
    pub last_text_len: usize,
    last_hit_map: HitMap,
    scroll_handle: ScrollHandle,
    pub cursor_offset: usize,
    pub selection_anchor: Option<usize>,
    on_hit_map_updated_callback: Option<
        Arc<Mutex<dyn FnMut(crate::typst_element::HitMap, &mut App) + Send + Sync + 'static>>,
    >,
    cursor_visible: bool,
    is_hovering_link: bool,
    _blink_task: Option<Task<()>>,
    compile_task: Option<Task<()>>,
    is_dragging: bool, // <-- Track mouse drag status
}

impl<W: TypstGpuiWorld> PreviewPanel<W> {
    /// Initialize the panel with a pre-configured World.
    pub fn new(world: Arc<Mutex<W>>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        let input_state = cx.new(|input_cx| {
            InputState::new(window, input_cx)
                .code_editor("typst")
                .multi_line(true)
                .soft_wrap(true)
                .line_number(false)
        });

        let subscription = cx.subscribe(
            &input_state,
            move |this_panel_ref: &mut PreviewPanel<W>,
                  emitting_input_state_entity: Entity<InputState>,
                  event: &InputEvent,
                  cx_for_panel: &mut Context<PreviewPanel<W>>| {
                if let InputEvent::Change = event {
                    let new_text = this_panel_ref
                        .input_state
                        .read(&cx_for_panel)
                        .text()
                        .to_string();
                    let new_len = new_text.len();

                    if !this_panel_ref.suppressing_events {
                        this_panel_ref.world.lock().set_source(new_text.clone());
                        this_panel_ref.compile(cx_for_panel);
                        cx_for_panel.emit(PreviewPanelEvent::SourceChanged(new_text));
                    }

                    this_panel_ref.last_text_len = new_len;

                    let current_cursor_offset =
                        emitting_input_state_entity.read(cx_for_panel).cursor();
                    this_panel_ref.cursor_offset = current_cursor_offset;

                    cx_for_panel.notify();
                }
            },
        );

        let preview_panel_entity_for_callback = cx.entity().clone();

        let on_hit_map_updated_callback_arc =
            Arc::new(Mutex::new(move |hit_map_data: HitMap, app_cx: &mut App| {
                let entity_for_update = preview_panel_entity_for_callback.clone();
                app_cx.update_entity(
                    &entity_for_update,
                    move |panel: &mut PreviewPanel<W>, _cx_update| {
                        panel.last_hit_map = hit_map_data;
                    },
                );
            }));

        let blink_task = cx.spawn(
            |view: WeakEntity<PreviewPanel<W>>, spawned_async_cx: &mut AsyncApp| {
                let mut cx = spawned_async_cx.clone();
                async move {
                    loop {
                        cx.background_executor()
                            .timer(Duration::from_millis(350))
                            .await;

                        let result = view.update(&mut cx, |this, cx| {
                            this.cursor_visible = !this.cursor_visible;
                            cx.notify();
                        });

                        if result.is_err() {
                            break;
                        }
                    }
                }
            },
        );

        // --- SINGLE SMART OBSERVER ---
        cx.observe(&input_state, |this, handle, cx| {
            let state = handle.read(cx);
            let new_cursor_offset = state.cursor();
            let sel = state.selected_range();

            this.cursor_offset = new_cursor_offset;

            if sel.is_empty() {
                if this.selection_anchor != Some(new_cursor_offset) {
                    this.selection_anchor = None;
                }
            } else {
                this.selection_anchor = if new_cursor_offset == sel.start {
                    Some(sel.end)
                } else {
                    Some(sel.start)
                };
            }

            // --- Synchronize to external workspace ---
            let range = if sel.is_empty() {
                new_cursor_offset..new_cursor_offset
            } else {
                sel
            };
            cx.emit(PreviewPanelEvent::SelectionChanged(range));

            cx.notify();
        })
        .detach();

        Self {
            world,
            document: None,
            render_state: Arc::new(TypstRenderState::default()),
            diagnostics: Vec::new(),
            focus_handle: focus_handle.clone(),
            zoom: 1.0,
            input_state,
            _input_state_subscription: Some(subscription),
            suppressing_events: false,
            last_text_len: 0,
            last_hit_map: crate::typst_element::HitMap::default(),
            scroll_handle: ScrollHandle::new(),
            cursor_offset: 0,
            selection_anchor: None,
            on_hit_map_updated_callback: Some(on_hit_map_updated_callback_arc),
            cursor_visible: true,
            is_hovering_link: false,
            _blink_task: Some(blink_task),
            compile_task: None,
            is_dragging: false,
        }
    }

    pub fn set_zoom(&mut self, zoom: f32, cx: &mut gpui::Context<Self>) {
        self.zoom = zoom.clamp(0.25, 5.0);
        cx.notify();
    }

    pub fn zoom_in(&mut self, cx: &mut gpui::Context<Self>) {
        self.set_zoom(self.zoom + 0.1, cx);
    }

    pub fn zoom_out(&mut self, cx: &mut gpui::Context<Self>) {
        self.set_zoom(self.zoom - 0.1, cx);
    }

    pub fn reset_zoom(&mut self, cx: &mut gpui::Context<Self>) {
        self.set_zoom(1.0, cx);
    }

    /// Update the Typst source code and trigger a re-render.
    pub fn set_source(&mut self, source: String, window: &mut Window, cx: &mut Context<Self>) {
        let source_for_input_state = source.clone();
        self.world.lock().set_source(source);

        // 1. Synchronously lock event emission to prevent feedback loops
        self.suppressing_events = true;

        let original_tab_stop_state = self.focus_handle.tab_stop;
        self.focus_handle.tab_stop = false;

        let current_selection = self.selection_range();

        self.input_state.update(cx, |input, input_cx| {
            input.set_value(source_for_input_state, window, input_cx);

            // Restore selection range
            if let Some(ref sel) = current_selection {
                input.set_selected_range(sel.clone(), input_cx);
                let new_pos = input.text().offset_to_position(sel.end);
                input.set_cursor_position(new_pos, window, input_cx);
            }
        });

        // 2. Unlock synchronously in the same frame/microtask
        self.suppressing_events = false;
        self.focus_handle.tab_stop = original_tab_stop_state;
        cx.notify();

        self.compile(cx);
    }

    /// Asynchronous compilation logic running on background thread.
    fn compile(&mut self, cx: &mut Context<Self>) {
        let world = self.world.clone();
        self.compile_task = None;

        let handle = cx.weak_entity();
        self.compile_task = Some(cx.spawn(|_view, spawned_async_cx: &mut AsyncApp| {
            let mut async_cx = spawned_async_cx.clone();
            async move {
                async_cx
                    .background_executor()
                    .timer(Duration::from_millis(50))
                    .await;

                let compiled_result = {
                    let world_guard = world.lock();
                    typst::compile(&*world_guard)
                };

                let _ = handle.update(&mut async_cx, |panel, cx| {
                    match compiled_result.output {
                        Ok(document) => {
                            let doc: Arc<PagedDocument> = Arc::new(document);
                            panel.world.lock().set_document(doc.clone());
                            panel.sync_fonts_to_gpui(&doc, cx);
                            panel.document = Some(doc);
                            panel.diagnostics.clear();
                            cx.emit(PreviewPanelEvent::DiagnosticsChanged(Vec::new()));
                        }
                        Err(errors) => {
                            let diags: Vec<_> = errors.into_iter().collect();
                            panel.diagnostics = diags.clone();
                            cx.emit(PreviewPanelEvent::DiagnosticsChanged(diags));
                        }
                    }
                    cx.notify();
                });
            }
        }));
    }

    pub fn update_document_info(
        &mut self,
        path: Option<std::path::PathBuf>,
        content: String,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.world
            .lock()
            .set_main_document_info(path, content.clone());
        cx.notify();
    }

    pub fn export_pdf(&self) -> Option<Vec<u8>> {
        self.document.as_ref().and_then(|doc| {
            let options = typst_pdf::PdfOptions::default();
            typst_pdf::pdf(doc, &options).ok()
        })
    }

    pub fn export_docx(&self) -> Option<Vec<u8>> {
        self.document
            .as_ref()
            .map(|doc| {
                let options = typsdocx::DocxOptions::default();
                typsdocx::docx(doc, &options)
            })
            .filter(|bytes| !bytes.is_empty())
    }

    fn sync_fonts_to_gpui(&mut self, document: &PagedDocument, cx: &mut Context<Self>) {
        let mut used_fonts = std::collections::HashSet::new();

        for page in document.pages() {
            self.collect_fonts_from_frame_recursive(&page.frame, &mut used_fonts);
        }

        let mut fonts_to_add = Vec::new();
        cx.update_global::<GpuiRegisteredFonts, _>(|cache, _| {
            for font in used_fonts {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                use std::hash::Hash;
                font.hash(&mut hasher);
                let id = std::hash::Hasher::finish(&hasher);

                if cache.0.insert(id) {
                    fonts_to_add.push(font);
                }
            }
        });

        if !fonts_to_add.is_empty() {
            let data_to_add: Vec<_> = fonts_to_add
                .iter()
                .map(|f| std::borrow::Cow::Owned(f.data().to_vec()))
                .collect();
            let _ = cx.text_system().add_fonts(data_to_add);
        }
    }

    fn collect_fonts_from_frame_recursive(
        &self,
        frame: &typst::layout::Frame,
        fonts_set: &mut std::collections::HashSet<typst::text::Font>,
    ) {
        for (_, item) in frame.items() {
            match item {
                typst::layout::FrameItem::Text(text) => {
                    fonts_set.insert(text.font.font().clone());
                }
                typst::layout::FrameItem::Group(group) => {
                    self.collect_fonts_from_frame_recursive(&group.frame, fonts_set);
                }
                _ => {}
            }
        }
    }

    pub fn hit_test(&self, _point_px: Point<Pixels>) -> Option<usize> {
        None
    }

    pub fn offset_for_point(&self, point_px: Point<Pixels>) -> Option<usize> {
        if self.last_hit_map.glyphs.is_empty() {
            return None;
        }

        let mut min_v_dist = f32::MAX;
        for glyph_info in &self.last_hit_map.glyphs {
            let bounds = glyph_info.bounds;
            let v_dist = if point_px.y < bounds.top() {
                (bounds.top() - point_px.y).as_f32()
            } else if point_px.y > bounds.bottom() {
                (point_px.y - bounds.bottom()).as_f32()
            } else {
                0.0
            };
            if v_dist < min_v_dist {
                min_v_dist = v_dist;
            }
        }

        let mut line_glyphs = Vec::new();
        for glyph_info in &self.last_hit_map.glyphs {
            let bounds = glyph_info.bounds;
            let v_dist = if point_px.y < bounds.top() {
                (bounds.top() - point_px.y).as_f32()
            } else if point_px.y > bounds.bottom() {
                (point_px.y - bounds.bottom()).as_f32()
            } else {
                0.0
            };

            if v_dist <= min_v_dist + 5.0 {
                line_glyphs.push(glyph_info);
            }
        }

        if line_glyphs.is_empty() {
            return None;
        }

        let mut closest_glyph = None;
        let mut min_h_dist = f32::MAX;

        for glyph in line_glyphs {
            let bounds = glyph.bounds;
            let h_dist = if point_px.x < bounds.left() {
                (bounds.left() - point_px.x).as_f32()
            } else if point_px.x > bounds.right() {
                (point_px.x - bounds.right()).as_f32()
            } else {
                0.0
            };

            if h_dist < min_h_dist {
                min_h_dist = h_dist;
                closest_glyph = Some(glyph);
            }
        }

        if let Some(glyph) = closest_glyph {
            let bounds = glyph.bounds;
            let center_x = bounds.left() + bounds.size.width / 2.0;
            if point_px.x > center_x {
                Some(glyph.byte_offset + glyph.byte_len)
            } else {
                Some(glyph.byte_offset)
            }
        } else {
            None
        }
    }

    pub fn selection_range(&self) -> Option<std::ops::Range<usize>> {
        self.selection_anchor.and_then(|anchor| {
            if anchor == self.cursor_offset {
                None
            } else {
                Some(anchor.min(self.cursor_offset)..anchor.max(self.cursor_offset))
            }
        })
    }

    pub fn set_selection(
        &mut self,
        range: std::ops::Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selection_anchor = Some(range.start);
        self.cursor_offset = range.end;

        self.input_state.update(cx, |input, input_cx| {
            input.set_selected_range(range.clone(), input_cx);
            let new_pos = input.text().offset_to_position(range.end);
            input.set_cursor_position(new_pos, window, input_cx);
        });
    }

    fn handle_link_click(&mut self, point: Point<Pixels>, cx: &mut Context<Self>) -> bool {
        for link in &self.last_hit_map.links {
            if link.bounds.contains(&point) {
                match &link.destination {
                    typst::model::Destination::Url(url) => {
                        let _ = gpui::App::open_url(cx, url.as_str());
                    }
                    typst::model::Destination::Location(loc) => {
                        self.scroll_to_location(*loc, cx);
                    }
                    typst::model::Destination::Position(_pos) => {}
                }
                return true;
            }
        }
        false
    }

    fn scroll_to_location(&mut self, loc: typst::introspection::Location, cx: &mut Context<Self>) {
        if let Some(anchor) = self.last_hit_map.anchors.iter().find(|a| a.location == loc) {
            let target_document_y = anchor.position.y;
            let padding = Pixels::from(20.0 * self.zoom);
            let scroll_offset = -(target_document_y - padding).max(Pixels::ZERO);
            self.scroll_handle
                .set_offset(Point::new(Pixels::ZERO, scroll_offset));
            cx.notify();
        }
    }
}

impl<W: TypstGpuiWorld> Render for PreviewPanel<W> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_focused = self.focus_handle.contains_focused(window, cx)
            || self
                .input_state
                .read(cx)
                .focus_handle(cx)
                .is_focused(window);

        gpui::div()
            .id("preview-panel-root")
            .size_full()
            .bg(rgb(0x1a1a1a))
            .track_focus(&self.focus_handle)
            .when(self.is_hovering_link, |this| {
                this.cursor(CursorStyle::PointingHand)
            })
            .when(is_focused, |this| {
                this.border_2().border_color(rgb(0x4a90e2))
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    if this.handle_link_click(event.position, cx) {
                        cx.stop_propagation();
                        return;
                    }

                    if let Some(byte_offset) = this.offset_for_point(event.position) {
                        this.is_dragging = true; // <-- Lock drag start
                        this.selection_anchor = Some(byte_offset);
                        this.cursor_offset = byte_offset;

                        this.input_state.update(cx, |input, input_cx| {
                            input.set_selected_range(byte_offset..byte_offset, input_cx);
                            let new_pos = input.text().offset_to_position(byte_offset);
                            input.set_cursor_position(new_pos, window, input_cx);
                        });
                    } else {
                        this.is_dragging = false;
                        this.selection_anchor = None;
                        this.input_state.update(cx, |input, input_cx| {
                            input.set_selected_range(0..0, input_cx);
                        });
                        cx.notify();
                    }

                    let input_focus_handle = this.input_state.read(cx).focus_handle(cx);
                    window.focus(&input_focus_handle, cx);
                    cx.notify();
                    cx.stop_propagation();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                let mut over_link = false;
                for link in &this.last_hit_map.links {
                    if link.bounds.contains(&event.position) {
                        over_link = true;
                        break;
                    }
                }

                if this.is_hovering_link != over_link {
                    this.is_hovering_link = over_link;
                    cx.notify();
                }

                if this.is_dragging {
                    // <-- Dependable drag select on drag
                    if let Some(byte_offset) = this.offset_for_point(event.position) {
                        this.cursor_offset = byte_offset;

                        if let Some(anchor) = this.selection_anchor {
                            this.input_state.update(cx, |input, input_cx| {
                                let normalized_range =
                                    anchor.min(byte_offset)..anchor.max(byte_offset);
                                input.set_selected_range(normalized_range, input_cx);
                            });
                        }
                        cx.notify();
                    }
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.is_dragging = false; // <-- Unlock drag
                    cx.stop_propagation();
                }),
            )
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                div().absolute().size_1().child(
                    Input::new(&self.input_state)
                        .absolute()
                        .top_0()
                        .left_0()
                        .w_full()
                        .h_full()
                        .text_color(transparent_black())
                        .bg(transparent_black())
                        .border_color(transparent_black())
                        .tab_index(-1),
                ),
            )
            .on_scroll_wheel(
                cx.listener(|this, event: &gpui::ScrollWheelEvent, _win, cx| {
                    if event.modifiers.control || event.modifiers.platform {
                        let delta = event.delta.pixel_delta(gpui::px(1.0)).y;
                        if delta > gpui::px(0.) {
                            this.set_zoom(this.zoom + 0.1, cx);
                        } else if delta < gpui::px(0.) {
                            this.set_zoom(this.zoom - 0.1, cx);
                        }
                    }
                }),
            )
            .child(
                gpui::div()
                    .id("preview-scroll-container")
                    .overflow_scroll()
                    .track_scroll(&self.scroll_handle)
                    .size_full()
                    .items_start()
                    .child(if let Some(doc) = &self.document {
                        let world_clone = self.world.clone();
                        let span_resolver = Some(std::sync::Arc::new(
                            move |span: typst::syntax::Span, offset: u16| {
                                if let Some(file_id) = span.id() {
                                    if let Ok(source) = world_clone.lock().source(file_id) {
                                        if let typst::syntax::SpanKind::Number { num, .. } =
                                            span.get()
                                        {
                                            if let Some(range) = source.range(num, None) {
                                                return range.start + offset as usize;
                                            }
                                        }
                                    }
                                }
                                0
                            },
                        )
                            as std::sync::Arc<
                                dyn Fn(typst::syntax::Span, u16) -> usize + Send + Sync,
                            >);

                        TypstElement::new(
                            doc.clone(),
                            self.render_state.clone(),
                            Some(self.cursor_offset),
                            self.selection_range(),
                            self.on_hit_map_updated_callback.clone(),
                            self.cursor_visible,
                            span_resolver,
                        )
                        .with_zoom(self.zoom)
                        .into_any_element()
                    } else {
                        gpui::div()
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(gpui::rgb(0x666666))
                            .child("No document compiled")
                            .into_any_element()
                    }),
            )
            .children(if !self.diagnostics.is_empty() {
                Some(
                    div()
                        .absolute()
                        .bottom_0()
                        .w_full()
                        .max_h(relative(0.5))
                        .bg(rgba(0x3d1a1a))
                        .overflow_y_scrollbar()
                        .p_4()
                        .children(self.diagnostics.iter().map(|diag| {
                            div()
                                .text_color(rgb(0xff4444))
                                .child(format!("Error: {}", diag.message))
                        })),
                )
            } else {
                None
            })
    }
}

impl<W: TypstGpuiWorld> gpui_component::dock::Panel for PreviewPanel<W> {
    fn panel_name(&self) -> &'static str {
        "PreviewPanel"
    }

    fn title(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child("Preview")
    }
}

impl<W: TypstGpuiWorld> EventEmitter<gpui_component::dock::PanelEvent> for PreviewPanel<W> {}

impl<W: TypstGpuiWorld> EventEmitter<PreviewPanelEvent> for PreviewPanel<W> {}

impl<W: TypstGpuiWorld> Focusable for PreviewPanel<W> {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

pub struct GpuiRegisteredFonts(pub std::collections::HashSet<u64>);
impl gpui::Global for GpuiRegisteredFonts {}
