use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    input::{Input, InputEvent, InputState, RopeExt},
    scroll::ScrollableElement,
};
use parking_lot::Mutex;
use std::{sync::Arc, time::Duration};
use typst::layout::PagedDocument;

pub mod typst_element;
use crate::typst_element::{HitMap, TypstElement, TypstRenderState};

/// Trait defining what the PreviewPanel needs from a Typst World.
pub trait TypstGpuiWorld: typst::World + Send + Sync + 'static {
    fn set_source(&mut self, source: String);
    fn set_main_document_info(&mut self, path: Option<std::path::PathBuf>, content: String);
}

#[derive(Debug, PartialEq, Eq)]
pub enum PreviewPanelEvent {
    // For Phase 1, we'll only handle appending a single character.
    // This will evolve in later phases for more complex edits (deletion, insertion at cursor, etc.).
    SourceChanged(String),
}

/// The PreviewPanel is a GPUI View that renders a Typst document.
pub struct PreviewPanel<W: TypstGpuiWorld> {
    world: Arc<Mutex<W>>,
    document: Option<std::sync::Arc<typst::layout::PagedDocument>>,
    pub render_state: Arc<TypstRenderState>,
    diagnostics: Vec<typst::diag::SourceDiagnostic>,
    focus_handle: FocusHandle,
    zoom: f32, // Add zoom field
    input_state: Entity<InputState>,
    _input_state_subscription: Option<Subscription>,
    pub suppressing_events: bool, // NEW: Flag to control event emission
    last_text_len: usize,
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
}

impl<W: TypstGpuiWorld> PreviewPanel<W> {
    /// Initialize the panel with a pre-configured World.
    pub fn new(world: Arc<Mutex<W>>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        let input_state = cx.new(|input_cx| {
            InputState::new(window, input_cx)
                .code_editor("typst") // CORRECTED: Call code_editor FIRST
                .multi_line(true) // Then multi_line (CodeEditor implies multi_line too, but explicit is fine)
                .soft_wrap(true)
                .line_number(false) // Now line_number can be called, as mode is CodeEditor
        });

        // Use cx.subscribe to listen for InputState events
        // CORRECTED: Listener signature adjusted to match Context::subscribe for Entity
        let subscription = cx.subscribe(
            &input_state,
            move |this_panel_ref: &mut PreviewPanel<W>, // `this_panel_ref` is &mut PreviewPanel
                  emitting_input_state_entity: Entity<InputState>, // `emitting_input_state_entity` is the Entity<InputState> that triggered the event
                  event: &InputEvent,
                  cx_for_panel: &mut Context<PreviewPanel<W>>| {
                if let InputEvent::Change = event {
                    let new_text = this_panel_ref
                        .input_state // Correct: Access input_state field on PreviewPanel
                        .read(&cx_for_panel)
                        .text()
                        .to_string();
                    let new_len = new_text.len();

                    // If the change originated from typing in THIS panel
                    if !this_panel_ref.suppressing_events {
                        // 1. Update the internal Typst world content
                        this_panel_ref.world.lock().set_source(new_text.clone());

                        // 2. Trigger a local re-compile so the preview updates
                        this_panel_ref.compile(cx_for_panel);

                        // 3. Emit the change to the Editor via main.rs bridge
                        cx_for_panel.emit(PreviewPanelEvent::SourceChanged(new_text));
                    }

                    // Always update the length tracking
                    this_panel_ref.last_text_len = new_len;

                    // ! CORRECTED: Get cursor offset directly from the emitting_input_state_entity !
                    // The `emitting_input_state_entity` is a `Model<InputState>` or `Entity<InputState>`.
                    // We can `read` it directly with `cx_for_panel` to get the current cursor.
                    let current_cursor_offset =
                        emitting_input_state_entity.read(cx_for_panel).cursor();
                    this_panel_ref.cursor_offset = current_cursor_offset;
                    // println!(
                    //     "DEBUG: PreviewPanel - Cursor offset updated to: {}",
                    //     this_panel_ref.cursor_offset
                    // );

                    cx_for_panel.notify(); // Ensure the panel re-renders with new cursor
                }
            },
        );

        //let current_cx_for_callback = cx.app().clone();
        let preview_panel_entity_for_callback = cx.entity().clone(); // Get Entity handle

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
                // 1. Clone the context so it can be moved into the async block
                let mut cx = spawned_async_cx.clone();

                async move {
                    loop {
                        // Wait for 500ms
                        cx.background_executor()
                            .timer(Duration::from_millis(350))
                            .await;

                        // 2. Use the cloned 'cx' (AsyncApp) to update the view
                        // update() on a WeakEntity returns an anyhow::Result
                        let result = view.update(&mut cx, |this, cx| {
                            this.cursor_visible = !this.cursor_visible;
                            cx.notify();
                        });

                        // 3. If the view was dropped (entity no longer exists), stop the loop
                        if result.is_err() {
                            break;
                        }
                    }
                }
            },
        );

        cx.observe(&input_state, |this, handle, cx| {
            let new_cursor_offset = handle.read(cx).cursor();
            if this.cursor_offset != new_cursor_offset {
                this.cursor_offset = new_cursor_offset;
                cx.notify();
            }
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
        }
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
        // println!(
        //     "DEBUG: PreviewPanel::set_source called. Source length: {}",
        //     source.len()
        // );

        let source_for_input_state = source.clone();
        self.world.lock().set_source(source);

        self.suppressing_events = true;
        // println!("DEBUG: PreviewPanel suppressing_events set to true in set_source");

        let preview_panel_entity = cx.entity().clone();

        // --- CORRECTED FOCUS MANAGEMENT ---
        // Store the original state of the PreviewPanel's focus_handle's tab_stop field
        let original_tab_stop_state = self.focus_handle.tab_stop;

        // Temporarily set the PreviewPanel's focus_handle's tab_stop field to false
        // This prevents the panel itself from being recognized as a tab stop, thus preventing
        // it from implicitly grabbing focus if it's not meant to be interacted with.
        self.focus_handle.tab_stop = false;
        // --- END CORRECTED FOCUS MANAGEMENT ---

        self.input_state.update(cx, |input, input_cx| {
            // Note: input.set_value() itself doesn't call focus().
            // It's input.set_cursor_position() that calls input.focus().
            // We need to prevent the input.focus() call inside set_cursor_position.
            // Since input.focus_handle is private, we can't disable it from here.
            // However, InputState.focus() will ultimately call its focus_handle().focus(window, cx),
            // which will check if the focus_handle has tab_stop(true).
            // If the *panel's* focus_handle is false, then the Input's focus_handle
            // should also be prevented IF the Input is a direct child relying on parent focus.
            // Let's try calling set_value, but avoid set_cursor_position here.
            input.set_value(source_for_input_state, window, input_cx);
        });

        // Use defer to reset the panel's suppressing_events and tab_stop flag
        cx.defer(move |app_cx| {
            app_cx.update_entity(&preview_panel_entity, |this_panel, cx_for_panel| {
                this_panel.suppressing_events = false;
                // Restore original tab_stop state for the PreviewPanel's focus_handle
                this_panel.focus_handle.tab_stop = original_tab_stop_state;
                // println!(
                //     "DEBUG: PreviewPanel suppressing_events and tab_stop reset via defer in set_source"
                // );
                cx_for_panel.notify();
            });
        });

        self.compile(cx);
    }

    /// Internal compilation logic.
    fn compile(&mut self, cx: &mut Context<Self>) {
        let world_guard = self.world.lock();

        match typst::compile(&*world_guard).output {
            Ok(document) => {
                let doc: Arc<PagedDocument> = Arc::new(document);

                drop(world_guard); // Release lock before sync_fonts_to_gpui if it needs its own lock
                self.sync_fonts_to_gpui(&doc, cx);
                self.document = Some(doc);
                self.diagnostics.clear();
            }
            Err(errors) => {
                self.diagnostics = errors.into_iter().collect();
                // We keep the old document visible if compilation fails,
                // or you could clear it: self.document = None;
            }
        }
        cx.notify();
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

    fn sync_fonts_to_gpui(
        &mut self,
        document: &typst::layout::PagedDocument,
        cx: &mut Context<Self>,
    ) {
        let mut used_fonts = std::collections::HashSet::new();

        // Directly call the recursive helper for each page
        for page in &document.pages {
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
                    fonts_set.insert(text.font.clone());
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
        // Iterate through the hit map in reverse to find the "frontmost" glyph if overlaps.

        for glyph_info in self.last_hit_map.glyphs.iter().rev() {
            if glyph_info.bounds.contains(&point_px) {
                return Some(glyph_info.byte_offset);
            }
        }
        None
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
                    typst::model::Destination::Position(_pos) => {
                        //Position Logic
                    }
                }
                return true;
            }
        }
        false
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
            .size_full()
            .bg(rgb(0x1a1a1a))
            .track_focus(&self.focus_handle)
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
                        this.selection_anchor = Some(byte_offset);
                        this.cursor_offset = byte_offset;

                        this.input_state.update(cx, |input, input_cx| {
                            let new_pos = input.text().offset_to_position(byte_offset);
                            input.set_cursor_position(new_pos, window, input_cx);
                        });
                    } else {
                        // Clear selection if clicking on empty space
                        this.selection_anchor = None;
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

                        // Sync with InputState so keyboard works from the drag end point
                        this.input_state.update(cx, |input, input_cx| {
                            let new_pos = input.text().offset_to_position(byte_offset);
                            input.set_cursor_position(new_pos, window, input_cx);
                        });
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
                    // .opacity(0.0) // Invisible but technically in the tree
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
                // Single container for both X and Y scrolling
                gpui::div()
                    .id("preview-scroll-container") // Giving it an ID helps GPUI track scroll state
                    .overflow_scroll()
                    .track_scroll(&self.scroll_handle)
                    .size_full()
                    // .overflow_y_scrollbar() // Enables both X and Y
                    .items_start()
                    .child(if let Some(doc) = &self.document {
                        // Calculate range from anchor and current cursor
                        let selection_range = self.selection_anchor.and_then(|anchor| {
                            if anchor == self.cursor_offset {
                                None
                            } else {
                                Some(anchor.min(self.cursor_offset)..anchor.max(self.cursor_offset))
                            }
                        });

                        TypstElement::new(
                            doc.clone(),
                            self.render_state.clone(),
                            // self.scroll_handle.offset(),
                            Some(self.cursor_offset),
                            selection_range,
                            self.on_hit_map_updated_callback.clone(),
                            self.cursor_visible,
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
