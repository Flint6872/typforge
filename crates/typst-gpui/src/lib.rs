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
    // For Phase 1, we'll only handle appending a single character.
    // This will evolve in later phases for more complex edits (deletion, insertion at cursor, etc.).
    SourceChanged(String),
    DiagnosticsChanged(Vec<typst::diag::SourceDiagnostic>),
    CursorMoved {
        offset: usize,
        selection: Option<std::ops::Range<usize>>,
    },
}

/// The PreviewPanel is a GPUI View that renders a Typst document.
pub struct PreviewPanel<W: TypstGpuiWorld> {
    world: Arc<Mutex<W>>,
    document: Option<std::sync::Arc<PagedDocument>>,
    pub render_state: Arc<TypstRenderState>,
    diagnostics: Vec<typst::diag::SourceDiagnostic>,
    focus_handle: FocusHandle,
    zoom: f32, // Add zoom field
    input_state: Entity<InputState>,
    _input_state_subscription: Option<Subscription>,
    pub suppressing_events: bool, // NEW: Flag to control event emission
    pub last_text_len: usize,
    last_hit_map: HitMap,
    scroll_handle: ScrollHandle,
    cursor_offset: usize,
    selection_anchor: Option<usize>,
    on_hit_map_updated_callback: Option<
        Arc<Mutex<dyn FnMut(crate::typst_element::HitMap, &mut App) + Send + Sync + 'static>>,
    >,
    cursor_visible: bool,
    is_hovering_link: bool,
    _blink_task: Option<Task<()>>,
    compile_task: Option<Task<()>>,
}

impl<W: TypstGpuiWorld> PreviewPanel<W> {
    /// Initialize the panel with a pre-configured World.
    pub fn new(world: Arc<Mutex<W>>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        let input_state = cx.new(|input_cx| {
            InputState::new(window, input_cx)
                .code_editor("typst") // CORRECTED: Call code_editor FIRST
                .multi_line(true) // Then multi_line (CodeEditor implies multi_line too, but explicit is fine)
                .soft_wrap(false)
                .line_number(false) // Now line_number can be called, as mode is CodeEditor
        });

        // Use cx.subscribe to listen for InputState events
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
            if this.suppressing_events {
                return;
            }

            let state = handle.read(cx);
            let new_cursor_offset = state.cursor();
            let sel = state.selected_range();

            if sel.is_empty() {
                if this.cursor_offset != new_cursor_offset || this.selection_anchor.is_none() {
                    this.selection_anchor = None;
                }
            } else {
                this.selection_anchor = if new_cursor_offset == sel.start {
                    Some(sel.end)
                } else {
                    Some(sel.start)
                };
            }

            let moved = this.cursor_offset != new_cursor_offset;
            this.cursor_offset = new_cursor_offset;

            if moved {
                this.notify_cursor_moved(cx);
            }
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
            _blink_task: Some(blink_task), // Store the task
            compile_task: None,
        }
    }

    pub fn notify_cursor_moved(&self, cx: &mut Context<Self>) {
        cx.emit(PreviewPanelEvent::CursorMoved {
            offset: self.cursor_offset,
            selection: self.selection_range(),
        });
    }

    pub fn set_zoom(&mut self, zoom: f32, cx: &mut gpui::Context<Self>) {
        self.zoom = zoom.clamp(0.25, 5.0);
        cx.notify();
    }

    pub fn zoom_in(&mut self, cx: &mut gpui::Context<Self>) {
        self.set_zoom(self.zoom + 0.1, cx);
    }

    /// Decrement the zoom level by 10%
    pub fn zoom_out(&mut self, cx: &mut gpui::Context<Self>) {
        self.set_zoom(self.zoom - 0.1, cx);
    }

    /// Reset zoom to 100%
    pub fn reset_zoom(&mut self, cx: &mut gpui::Context<Self>) {
        self.set_zoom(1.0, cx);
    }

    /// Update the Typst source code and trigger a re-render.

    pub fn set_source(&mut self, source: String, window: &mut Window, cx: &mut Context<Self>) {
        let source_for_input_state = source.clone();
        self.world.lock().set_source(source);

        self.suppressing_events = true;

        let preview_panel_entity = cx.entity().clone();
        let original_tab_stop_state = self.focus_handle.tab_stop;
        self.focus_handle.tab_stop = false;

        // 1. Preserve active selection anchor and cursor offset before set_value clears them
        let saved_anchor = self.selection_anchor;
        let saved_cursor = self.cursor_offset;
        let current_selection = self.selection_range();

        self.input_state.update(cx, |input, input_cx| {
            input.set_value(source_for_input_state, window, input_cx);

            // 2. Restore selection range on the input state
            if let Some(ref sel) = current_selection {
                input.set_selected_range(sel.clone(), input_cx);
                let new_pos = input.text().offset_to_position(sel.end);
                input.set_cursor_position(new_pos, window, input_cx);
            }
        });

        // 3. Restore PreviewPanel's internal anchor and cursor
        self.selection_anchor = saved_anchor;
        self.cursor_offset = saved_cursor;

        cx.defer(move |app_cx| {
            app_cx.update_entity(&preview_panel_entity, |this_panel, cx_for_panel| {
                this_panel.suppressing_events = false;
                this_panel.focus_handle.tab_stop = original_tab_stop_state;
                cx_for_panel.notify();
            });
        });

        self.compile(cx);
    }

    /// Asynchronous compilation logic running on background thread.
    fn compile(&mut self, cx: &mut Context<Self>) {
        let world = self.world.clone();

        // Cancel any pending compilation task to debounce rapid typing
        self.compile_task = None;

        let handle = cx.weak_entity();
        self.compile_task = Some(cx.spawn(|_view, spawned_async_cx: &mut AsyncApp| {
            let mut async_cx = spawned_async_cx.clone();
            async move {
                // Debounce compile requests by 50ms during continuous typing
                async_cx
                    .background_executor()
                    .timer(Duration::from_millis(50))
                    .await;

                // Perform heavy CPU-bound compile on background executor
                let start_compile = std::time::Instant::now();
                let compiled_result = {
                    let world_guard = world.lock();
                    typst::compile(&*world_guard)
                };
                println!(
                    "BENCHMARK: typst::compile took {:?}",
                    start_compile.elapsed()
                );

                // Jump back to the main thread to update the UI
                let _ = handle.update(&mut async_cx, |panel, cx| {
                    match compiled_result.output {
                        Ok(document) => {
                            let doc: Arc<PagedDocument> = Arc::new(document);

                            // Sync document back to the shared world
                            panel.world.lock().set_document(doc.clone());

                            // Sync fonts
                            let start_fonts = std::time::Instant::now();
                            panel.sync_fonts_to_gpui(&doc, cx);
                            println!(
                                "BENCHMARK: sync_fonts_to_gpui took {:?}",
                                start_fonts.elapsed()
                            );

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

    /// Updates the GpuiWorld's main document path and content.
    pub fn update_document_info(
        &mut self,
        path: Option<std::path::PathBuf>,
        content: String,
        _window: &mut Window, // Marked as unused
        cx: &mut Context<Self>,
    ) {
        // println!(
        //     "DEBUG: PreviewPanel::update_document_info called. Content length: {}",
        //     content.len()
        // );
        self.world
            .lock()
            .set_main_document_info(path, content.clone());

        // REMOVED redundant input_state.update here.
        // It is already handled by set_source in main.rs.

        cx.notify();
    }

    pub fn export_pdf(&self) -> Option<Vec<u8>> {
        self.document.as_ref().and_then(|doc| {
            let options = typst_pdf::PdfOptions::default();
            typst_pdf::pdf(doc, &options).ok()
        })
    }

    /// Exports the document to DOCX bytes using the typsdocx crate.
    pub fn export_docx(&self) -> Option<Vec<u8>> {
        self.document
            .as_ref()
            .map(|doc| {
                // Using the new typsdocx crate
                let options = typsdocx::DocxOptions::default();
                typsdocx::docx(doc, &options)
            })
            .filter(|bytes| !bytes.is_empty())
        // filter ensures we return None if the Vec is empty,
        // triggering your error message in main.rs
    }

    fn sync_fonts_to_gpui(&mut self, document: &PagedDocument, cx: &mut Context<Self>) {
        let mut used_fonts = std::collections::HashSet::new();

        // Directly call the recursive helper for each page
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
            println!(
                "DEBUG: Registered {} new document fonts with GPUI",
                fonts_to_add.len()
            );
        }
    }

    // New private helper for recursive calls, if you want to keep the recursion pattern.
    // If not, simply inline the group handling too.
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

        // 1. Find the minimum vertical distance from our cursor Y to any glyph's line span.
        let mut min_v_dist = f32::MAX;
        for glyph_info in &self.last_hit_map.glyphs {
            let bounds = glyph_info.bounds;
            let v_dist = if point_px.y < bounds.top() {
                (bounds.top() - point_px.y).as_f32()
            } else if point_px.y > bounds.bottom() {
                (point_px.y - bounds.bottom()).as_f32()
            } else {
                0.0 // Mouse is vertically inside this line
            };
            if v_dist < min_v_dist {
                min_v_dist = v_dist;
            }
        }

        // 2. Collect all glyphs that belong to this closest vertical line (within a 5px threshold).
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

        // 3. Find the horizontally closest glyph on this specific line.
        let mut closest_glyph = None;
        let mut min_h_dist = f32::MAX;

        for glyph in line_glyphs {
            let bounds = glyph.bounds;
            let h_dist = if point_px.x < bounds.left() {
                (bounds.left() - point_px.x).as_f32()
            } else if point_px.x > bounds.right() {
                (point_px.x - bounds.right()).as_f32()
            } else {
                0.0 // Mouse is horizontally inside this character
            };

            if h_dist < min_h_dist {
                min_h_dist = h_dist;
                closest_glyph = Some(glyph);
            }
        }

        // 4. Return the correct offset (before or after the character)
        if let Some(glyph) = closest_glyph {
            let bounds = glyph.bounds;
            let center_x = bounds.left() + bounds.size.width / 2.0;
            if point_px.x > center_x {
                // If clicked on the right half of the character, place cursor after it
                Some(glyph.byte_offset + glyph.byte_len)
            } else {
                // If clicked on the left half, place cursor before it
                Some(glyph.byte_offset)
            }
        } else {
            None
        }
    }

    /// Returns the active selection range normalized (min..max) if one exists.
    pub fn selection_range(&self) -> Option<std::ops::Range<usize>> {
        self.selection_anchor.and_then(|anchor| {
            if anchor == self.cursor_offset {
                None
            } else {
                Some(anchor.min(self.cursor_offset)..anchor.max(self.cursor_offset))
            }
        })
    }

    /// Explicitly updates the selection anchor and cursor position, syncing the underlying InputState.
    pub fn set_selection(
        &mut self,
        range: std::ops::Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selection_anchor = Some(range.start);
        self.cursor_offset = range.end;

        self.suppressing_events = true;
        self.input_state.update(cx, |input, input_cx| {
            input.set_selected_range(range.clone(), input_cx);
            let new_pos = input.text().offset_to_position(range.end);
            input.set_cursor_position(new_pos, window, input_cx);
        });
        self.suppressing_events = false;

        self.notify_cursor_moved(cx);
        cx.notify();
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
                    typst::model::Destination::Position(pos) => {
                        self.scroll_to_page_position(*pos, cx);
                    }
                }
                return true;
            }
        }
        false
    }

    fn scroll_to_page_position(
        &mut self,
        pos: typst::introspection::PagedPosition,
        cx: &mut Context<Self>,
    ) {
        if let Some(doc) = &self.document {
            let scale_factor = (96.0 / 72.0) * self.zoom;
            let page_margin_px = gpui::px(20.0 * self.zoom);
            let mut target_y = Pixels::ZERO;

            let target_page_idx = pos.page.get().saturating_sub(1);
            for (i, page) in doc.pages().iter().enumerate() {
                if i == target_page_idx {
                    target_y += Pixels::from(pos.point.y.to_pt() as f32 * scale_factor);
                    break;
                }
                let page_h = Pixels::from(page.frame.height().to_pt() as f32 * scale_factor);
                target_y += page_h + page_margin_px;
            }

            let padding = Pixels::from(20.0 * self.zoom);
            let scroll_offset = -(target_y - padding).max(Pixels::ZERO);
            self.scroll_handle
                .set_offset(Point::new(Pixels::ZERO, scroll_offset));
            cx.notify();
        }
    }

    fn scroll_to_location(&mut self, loc: typst::introspection::Location, cx: &mut Context<Self>) {
        if let Some(anchor) = self.last_hit_map.anchors.iter().find(|a| a.location == loc) {
            // This is now the physical distance in pixels from the start of the file.
            let target_document_y = anchor.position.y;

            // Breathing room: subtract 20px so the heading isn't touching the window edge.
            let padding = Pixels::from(20.0 * self.zoom);

            // To show the target at the top, we set a NEGATIVE offset.
            let scroll_offset = -(target_document_y - padding).max(Pixels::ZERO);

            println!("STABLE JUMP TO: {}", scroll_offset);

            self.scroll_handle
                .set_offset(Point::new(Pixels::ZERO, scroll_offset));
            cx.notify();
        }
    }
}

impl<W: TypstGpuiWorld> Render for PreviewPanel<W> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_focused = self.focus_handle.contains_focused(window, cx);

        gpui::div()
            .id("preview-panel-root")
            .relative()
            .size_full()
            .bg(rgb(0x1a1a1a))
            .track_focus(&self.focus_handle)
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let key = event.keystroke.key.to_lowercase();
                match key.as_str() {
                    "left" | "arrowleft" => {
                        if let Some(next_offset) = this.prev_visual_offset(this.cursor_offset) {
                            this.set_selection(next_offset..next_offset, window, cx);
                            cx.stop_propagation();
                        }
                    }
                    "right" | "arrowright" => {
                        if let Some(next_offset) = this.next_visual_offset(this.cursor_offset) {
                            this.set_selection(next_offset..next_offset, window, cx);
                            cx.stop_propagation();
                        }
                    }
                    "up" | "arrowup" => {
                        if let Some(next_offset) = this.up_visual_offset(this.cursor_offset) {
                            this.set_selection(next_offset..next_offset, window, cx);
                            cx.stop_propagation();
                        }
                    }
                    "down" | "arrowdown" => {
                        if let Some(next_offset) = this.down_visual_offset(this.cursor_offset) {
                            this.set_selection(next_offset..next_offset, window, cx);
                            cx.stop_propagation();
                        }
                    }
                    _ => {}
                }
            }))
            .when(self.is_hovering_link, |this| {
                this.cursor(CursorStyle::PointingHand)
            })
            .when(is_focused, |this| {
                this.border_2().border_color(rgb(0x4a90e2)) // Blue border when focused
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    if this.handle_link_click(event.position, cx) {
                        cx.stop_propagation();
                        return;
                    }

                    if let Some(byte_offset) = this.offset_for_point(event.position) {
                        let text = this.input_state.read(cx).text().to_string();

                        match event.click_count {
                            1 => {
                                // Single-click: standard cursor placement
                                this.selection_anchor = Some(byte_offset);
                                this.cursor_offset = byte_offset;

                                this.suppressing_events = true;
                                this.input_state.update(cx, |input, input_cx| {
                                    input.set_selected_range(byte_offset..byte_offset, input_cx);
                                    let new_pos = input.text().offset_to_position(byte_offset);
                                    input.set_cursor_position(new_pos, window, input_cx);
                                });
                                this.suppressing_events = false;
                            }
                            2 => {
                                // Double-click: select word
                                let range = find_word_boundaries(&text, byte_offset);
                                this.selection_anchor = Some(range.start);
                                this.cursor_offset = range.end;

                                this.suppressing_events = true;
                                this.input_state.update(cx, |input, input_cx| {
                                    input.set_selected_range(range.clone(), input_cx);
                                    let new_pos = input.text().offset_to_position(range.end);
                                    input.set_cursor_position(new_pos, window, input_cx);
                                });
                                this.suppressing_events = false;
                            }
                            3 => {
                                // Triple-click: select paragraph
                                let range = find_paragraph_boundaries(&text, byte_offset);
                                this.selection_anchor = Some(range.start);
                                this.cursor_offset = range.end;

                                this.suppressing_events = true;
                                this.input_state.update(cx, |input, input_cx| {
                                    input.set_selected_range(range.clone(), input_cx);
                                    let new_pos = input.text().offset_to_position(range.end);
                                    input.set_cursor_position(new_pos, window, input_cx);
                                });
                                this.suppressing_events = false;
                            }
                            _ => {}
                        }
                        this.notify_cursor_moved(cx);
                    } else {
                        // Clear selection if clicking on empty space
                        this.selection_anchor = None;
                        this.suppressing_events = true;
                        this.input_state.update(cx, |input, input_cx| {
                            input.set_selected_range(0..0, input_cx);
                        });
                        this.suppressing_events = false;
                        this.notify_cursor_moved(cx);
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

                if event.pressed_button == Some(MouseButton::Left) {
                    if let Some(byte_offset) = this.offset_for_point(event.position) {
                        this.cursor_offset = byte_offset;

                        if let Some(anchor) = this.selection_anchor {
                            this.suppressing_events = true;
                            this.input_state.update(cx, |input, input_cx| {
                                let normalized_range =
                                    anchor.min(byte_offset)..anchor.max(byte_offset);
                                // This sets both selection bounds AND updates the cursor without collapsing!
                                input.set_selected_range(normalized_range, input_cx);
                                let new_pos = input.text().offset_to_position(byte_offset);
                                input.set_cursor_position(new_pos, window, input_cx);
                            });
                            this.suppressing_events = false;
                        }
                        this.notify_cursor_moved(cx);
                        cx.notify();
                    }
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| cx.stop_propagation()),
            )
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .absolute()
                    // --- FIX 3: Ensure the input "exists" for the focus system ---
                    .size_1()
                    .child(
                        Input::new(&self.input_state)
                            .absolute() // Allow us to position it precisely
                            .top_0() // Start at top-left of the wrapper div
                            .left_0()
                            .w_full() // Take full width/height for layout calculations, but we'll override visual.
                            .h_full()
                            .text_color(transparent_black()) // Make the actual input text transparent
                            .bg(transparent_black())
                            .border_color(transparent_black()) // Make the input's border transparent
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
                // Stationary wrapper that hosts the scrollbar so it stays in place
                gpui::div()
                    .id("preview-scroll-wrapper")
                    .relative()
                    .h_5_6()
                    .vertical_scrollbar(&self.scroll_handle)
                    .child(
                        // The actual scrolling container that handles the viewport and tracking
                        gpui::div()
                            .id("preview-scroll-container")
                            .overflow_scroll()
                            .track_scroll(&self.scroll_handle)
                            .size_full()
                            .items_start()
                            .child(if let Some(doc) = &self.document {
                                // Create the resolver closure accessing the world
                                let world_clone = self.world.clone();
                                let span_resolver = Some(std::sync::Arc::new(
                                    move |span: typst::syntax::Span, offset: u16| {
                                        if let Some(file_id) = span.id() {
                                            if let Ok(source) = world_clone.lock().source(file_id) {
                                                // Use .get() to access the internal data of the Span
                                                if let typst::syntax::SpanKind::Number {
                                                    num, ..
                                                } = span.get()
                                                {
                                                    // Now you have the SpanNumber (num) to pass to range()
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
                                    span_resolver, // Pass the resolver here
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
                    ),
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

// Support for gpui_component's Docking system
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

impl<W: TypstGpuiWorld> PreviewPanel<W> {
    pub fn next_visual_offset(&self, current: usize) -> Option<usize> {
        let mut glyphs = self.last_hit_map.glyphs.clone();
        if glyphs.is_empty() {
            return None;
        }
        glyphs.sort_by_key(|g| g.byte_offset);

        for glyph in &glyphs {
            if glyph.byte_offset > current {
                return Some(glyph.byte_offset);
            }
        }

        // Fallback: If at the last visual glyph, place caret at its boundary end
        if let Some(last) = glyphs.last() {
            if current < last.byte_offset + last.byte_len {
                return Some(last.byte_offset + last.byte_len);
            }
        }
        None
    }

    pub fn prev_visual_offset(&self, current: usize) -> Option<usize> {
        let mut glyphs = self.last_hit_map.glyphs.clone();
        if glyphs.is_empty() {
            return None;
        }
        glyphs.sort_by_key(|g| g.byte_offset);

        for glyph in glyphs.iter().rev() {
            if glyph.byte_offset < current {
                return Some(glyph.byte_offset);
            }
        }
        None
    }

    pub fn cursor_point_pixels(&self) -> Option<Point<Pixels>> {
        let glyphs = &self.last_hit_map.glyphs;
        if glyphs.is_empty() {
            return None;
        }

        for g in glyphs {
            if self.cursor_offset >= g.byte_offset
                && self.cursor_offset < g.byte_offset + g.byte_len
            {
                return Some(g.bounds.origin);
            }
        }

        // Fallback: End of document
        if let Some(last) = glyphs.last() {
            if self.cursor_offset >= last.byte_offset + last.byte_len {
                return Some(last.bounds.top_right());
            }
        }

        None
    }

    pub fn down_visual_offset(&self, _current: usize) -> Option<usize> {
        let glyphs = &self.last_hit_map.glyphs;
        if glyphs.is_empty() {
            return None;
        }

        let cursor_pt = self.cursor_point_pixels()?;
        let mut closest_below_top: Option<Pixels> = None;

        // Find the next unique visual line vertical coordinate below current caret Y
        for g in glyphs {
            let top = g.bounds.top();
            if top > cursor_pt.y + Pixels::from(2.0) {
                if let Some(best) = closest_below_top {
                    if top < best {
                        closest_below_top = Some(top);
                    }
                } else {
                    closest_below_top = Some(top);
                }
            }
        }

        let row_top = closest_below_top?;
        let mut row_glyphs = Vec::new();
        for g in glyphs {
            if (g.bounds.top() - row_top).abs() < Pixels::from(5.0) {
                row_glyphs.push(g);
            }
        }

        // Find the glyph horizontally closest to our X column
        let closest_g = row_glyphs.into_iter().min_by_key(|g| {
            let center_x = g.bounds.left() + g.bounds.size.width / 2.0;
            let dist = (center_x - cursor_pt.x).abs();
            (dist.as_f32() * 1000.0) as i32
        })?;

        Some(closest_g.byte_offset)
    }

    pub fn up_visual_offset(&self, _current: usize) -> Option<usize> {
        let glyphs = &self.last_hit_map.glyphs;
        if glyphs.is_empty() {
            return None;
        }

        let cursor_pt = self.cursor_point_pixels()?;
        let mut closest_above_bottom: Option<Pixels> = None;

        // Find the next unique visual line vertical coordinate above current caret Y
        for g in glyphs {
            let bottom = g.bounds.bottom();
            if bottom < cursor_pt.y - Pixels::from(2.0) {
                if let Some(best) = closest_above_bottom {
                    if bottom > best {
                        closest_above_bottom = Some(bottom);
                    }
                } else {
                    closest_above_bottom = Some(bottom);
                }
            }
        }

        let row_bottom = closest_above_bottom?;
        let mut row_glyphs = Vec::new();
        for g in glyphs {
            if (g.bounds.bottom() - row_bottom).abs() < Pixels::from(5.0) {
                row_glyphs.push(g);
            }
        }

        // Find the glyph horizontally closest to our X column
        let closest_g = row_glyphs.into_iter().min_by_key(|g| {
            let center_x = g.bounds.left() + g.bounds.size.width / 2.0;
            let dist = (center_x - cursor_pt.x).abs();
            (dist.as_f32() * 1000.0) as i32
        })?;

        Some(closest_g.byte_offset)
    }
}

// Helper to find the byte boundaries of the word under a given offset
fn find_word_boundaries(text: &str, offset: usize) -> std::ops::Range<usize> {
    if text.is_empty() {
        return 0..0;
    }
    let clamped = offset.min(text.len());

    let chars: Vec<(usize, char)> = text.char_indices().collect();
    if chars.is_empty() {
        return 0..0;
    }

    // Find the index of the character at our offset
    let mut char_idx = chars.len();
    for (i, (idx, _)) in chars.iter().enumerate() {
        if *idx >= clamped {
            char_idx = i;
            break;
        }
    }
    if char_idx > 0 && char_idx == chars.len() {
        char_idx = chars.len() - 1;
    }

    let is_word_char = |c: char| c.is_alphanumeric() || c == '_';

    // Scan backward
    let mut start_idx = char_idx;
    while start_idx > 0 && is_word_char(chars[start_idx - 1].1) {
        start_idx -= 1;
    }

    // Scan forward
    let mut end_idx = char_idx;
    while end_idx < chars.len() && is_word_char(chars[end_idx].1) {
        end_idx += 1;
    }

    let start_byte = chars.get(start_idx).map(|(idx, _)| *idx).unwrap_or(0);
    let end_byte = chars
        .get(end_idx)
        .map(|(idx, _)| *idx)
        .unwrap_or(text.len());

    start_byte..end_byte
}

// Helper to find the byte boundaries of the paragraph under a given offset
fn find_paragraph_boundaries(text: &str, offset: usize) -> std::ops::Range<usize> {
    if text.is_empty() {
        return 0..0;
    }
    let clamped = offset.min(text.len());

    // A paragraph is bounded by double newlines or file boundaries
    let mut start = 0;
    let before = &text[..clamped];
    if let Some(pos) = before.rfind("\n\n") {
        start = pos + 2;
    } else if let Some(pos) = before.rfind("\r\n\r\n") {
        start = pos + 4;
    }

    let mut end = text.len();
    let after = &text[clamped..];
    if let Some(pos) = after.find("\n\n") {
        end = clamped + pos;
    } else if let Some(pos) = after.find("\r\n\r\n") {
        end = clamped + pos;
    }

    start..end
}
