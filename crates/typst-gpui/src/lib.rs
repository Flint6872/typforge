use crate::typst_element::{HitMap, TypstElement, TypstRenderState};
use gpui_kit::component::{StyledExt, scroll::ScrollableElement};
use gpui_kit::{prelude::FluentBuilder, *};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::{sync::Arc, time::Duration};
use typastry::spellcheck::{
    Dictionary, TextDiagnostic,
    checker::{HarperCache, check_source_with_harper},
    dictionary::DictionaryManager,
    extractor::detect_document_language,
};
use typst_layout::PagedDocument;

pub mod canvas_input;
pub mod caret_geometry;
pub mod typst_element;

/// Trait defining what the PreviewPanel needs from a Typst World.
pub trait TypstGpuiWorld: typst::World + Send + Sync + 'static {
    fn set_source(&mut self, source: String);
    fn set_main_document_info(&mut self, path: Option<std::path::PathBuf>, content: String);

    fn document(&self) -> Option<std::sync::Arc<PagedDocument>> {
        None
    }
    fn set_document(&mut self, _doc: std::sync::Arc<PagedDocument>) {}
    fn main_file_id(&self) -> typst::syntax::FileId;
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
    UndoRequested,
    RedoRequested,
}

/// The PreviewPanel is a GPUI View that renders a Typst document.
pub struct PreviewPanel<W: TypstGpuiWorld> {
    world: Arc<Mutex<W>>,
    document: Option<std::sync::Arc<PagedDocument>>,
    pub render_state: Arc<TypstRenderState>,

    focus_handle: FocusHandle,
    zoom: f32, // Add zoom field
    pub source: String,

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
    is_compiling: bool,
    pending_compile_source: Option<String>,
    pending_compile_path: Option<Option<std::path::PathBuf>>,

    // --- NEW SPELLCHECK & DIAGNOSTICS STATE FIELDS ---
    pub diagnostics: Vec<TextDiagnostic>,
    spellcheck_task: Option<Task<()>>,
    pub dictionary_manager: Arc<Mutex<DictionaryManager>>,
    cached_dictionary: Option<(String, Arc<Dictionary>)>,
    cached_harper: Option<HarperCache>,
    cached_sources: Arc<HashMap<typst::syntax::FileId, Arc<typst_syntax::Source>>>,
    cached_span_resolver:
        Option<Arc<dyn Fn(typst::syntax::Span, u16) -> usize + Send + Sync + 'static>>,

    pub active_menu_diagnostics: Vec<TextDiagnostic>,
    // --- NEW WYSIWYG STATE FIELDS Visual editing & context menu---
    pub pending_transaction: Option<typastry::wysiwyg::MutationTransaction>,
    pub visual_edit_error: Option<String>,
    pub inspected_element: Option<typastry::wysiwyg::ElementProperties>,
}

impl<W: TypstGpuiWorld> PreviewPanel<W> {
    /// Initialize the panel with a pre-configured World.
    pub fn new(world: Arc<Mutex<W>>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

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
                            .timer(Duration::from_millis(450))
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

        let dict_dir = std::env::current_dir()
            .unwrap_or_default()
            .join("assets")
            .join("dictionaries");
        let dictionary_manager = Arc::new(Mutex::new(DictionaryManager::new(dict_dir))); // Local variable

        Self {
            world,
            document: None,
            render_state: Arc::new(TypstRenderState::default()),

            focus_handle,
            zoom: 1.0,
            source: String::new(),
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
            is_compiling: false,
            pending_compile_source: None,
            pending_compile_path: None,

            diagnostics: Vec::new(),
            spellcheck_task: None,
            dictionary_manager,
            cached_dictionary: None,
            cached_harper: None,
            cached_sources: Arc::new(HashMap::new()),
            cached_span_resolver: None,

            active_menu_diagnostics: Vec::new(),
            pending_transaction: None,
            visual_edit_error: None,
            inspected_element: None,
        }
    }

    pub fn notify_cursor_moved(&self, cx: &mut Context<Self>) {
        cx.emit(PreviewPanelEvent::CursorMoved {
            offset: self.cursor_offset,
            selection: self.selection_range(),
        });
    }

    pub fn set_zoom(&mut self, zoom: f32, cx: &mut gpui_kit::Context<Self>) {
        self.zoom = zoom.clamp(0.25, 5.0);
        cx.notify();
    }

    pub fn zoom_in(&mut self, cx: &mut gpui_kit::Context<Self>) {
        self.set_zoom(self.zoom + 0.1, cx);
    }

    /// Decrement the zoom level by 10%
    pub fn zoom_out(&mut self, cx: &mut gpui_kit::Context<Self>) {
        self.set_zoom(self.zoom - 0.1, cx);
    }

    /// Reset zoom to 100%
    pub fn reset_zoom(&mut self, cx: &mut gpui_kit::Context<Self>) {
        self.set_zoom(1.0, cx);
    }

    /// Update the Typst source code and trigger a re-render.

    pub fn set_source(&mut self, source: String, _window: &mut Window, cx: &mut Context<Self>) {
        if self.source == source {
            return;
        }
        self.source = source.clone();
        self.last_text_len = source.len();
        self.pending_compile_source = Some(source);
        self.compile(cx);
        self.schedule_spellcheck(cx);
        cx.notify();
    }

    pub fn set_selection(
        &mut self,
        range: std::ops::Range<usize>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selection_anchor = Some(range.start);
        self.cursor_offset = range.end;

        self.notify_cursor_moved(cx);
        cx.notify();
    }

    /// Asynchronous compilation logic running on background thread (single-flight coalescing).
    fn compile(&mut self, cx: &mut Context<Self>) {
        if self.is_compiling {
            return;
        }

        let Some(source_to_compile) = self.pending_compile_source.take() else {
            return;
        };
        let pending_path = self.pending_compile_path.take();

        self.is_compiling = true;
        let world = self.world.clone();
        let handle = cx.weak_entity();

        cx.spawn(move |_view, spawned_async_cx: &mut AsyncApp| {
            let mut async_cx = spawned_async_cx.clone();
            async move {
                let world_for_bg = world.clone();
                let (compiled_result, cached_sources_arc, used_fonts) = async_cx
                    .background_executor()
                    .spawn(async move {
                        let start_compile = std::time::Instant::now();
                        let compiled_result: typst::diag::Warned<
                            typst::diag::SourceResult<PagedDocument>,
                        > = {
                            let mut world_guard = world_for_bg.lock();

                            // Apply path update in the background if we have one
                            if let Some(path) = pending_path {
                                world_guard.set_main_document_info(path, source_to_compile.clone());
                            }

                            world_guard.set_source(source_to_compile);
                            typst::compile(&*world_guard)
                        };
                        println!(
                            "BENCHMARK: typst::compile took {:?}",
                            start_compile.elapsed()
                        );

                        let mut collected_sources_map = HashMap::new();
                        let mut used_fonts = std::collections::HashSet::new();

                        if let Ok(ref document) = compiled_result.output {
                            let mut referenced_ids = std::collections::HashSet::new();
                            for page in document.pages() {
                                collect_file_ids_from_frame_recursive(
                                    &page.frame,
                                    &mut referenced_ids,
                                );
                                collect_fonts_from_frame_recursive_static(
                                    &page.frame,
                                    &mut used_fonts,
                                );
                            }

                            let world_guard = world_for_bg.lock();
                            let main_id = world_guard.main_file_id();
                            if let Ok(main_source) = world_guard.source(main_id) {
                                collected_sources_map.insert(main_id, Arc::new(main_source));
                            }
                            for id in referenced_ids {
                                if !collected_sources_map.contains_key(&id) {
                                    if let Ok(source) = world_guard.source(id) {
                                        collected_sources_map.insert(id, Arc::new(source));
                                    }
                                }
                            }
                        }

                        (compiled_result, Arc::new(collected_sources_map), used_fonts)
                    })
                    .await;

                // Jump back to the main thread to update the UI
                let _ = handle.update(&mut async_cx, |panel, cx| {
                    panel.is_compiling = false;

                    let mut all_diags: Vec<typst::diag::SourceDiagnostic> =
                        compiled_result.warnings.into_iter().collect();

                    match compiled_result.output {
                        Ok(document) => {
                            let doc: Arc<PagedDocument> = Arc::new(document);
                            panel.sync_fonts_set_to_gpui(used_fonts, cx);

                            let span_sources = cached_sources_arc.clone();
                            let world_fallback = panel.world.clone();
                            panel.cached_sources = cached_sources_arc;
                            panel.cached_span_resolver =
                                Some(Arc::new(move |span: typst::syntax::Span, offset: u16| {
                                    if let Some(file_id) = span.id() {
                                        if let Some(source) = span_sources.get(&file_id) {
                                            if let typst::syntax::SpanKind::Number { num, .. } =
                                                span.get()
                                            {
                                                if let Some(range) = source.range(num, None) {
                                                    return range.start + offset as usize;
                                                }
                                            }
                                        } else if let Ok(source) =
                                            world_fallback.lock().source(file_id)
                                        {
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
                                }));

                            panel.document = Some(doc);
                            cx.emit(PreviewPanelEvent::DiagnosticsChanged(all_diags));
                            panel.pending_transaction = None;
                        }
                        Err(errors) => {
                            all_diags.extend(errors);
                            cx.emit(PreviewPanelEvent::DiagnosticsChanged(all_diags.clone()));

                            if let Some(_tx) = panel.pending_transaction.take() {
                                let error_msg = all_diags
                                    .iter()
                                    .find(|d| d.severity == typst::diag::Severity::Error)
                                    .map(|d| d.message.to_string())
                                    .unwrap_or_else(|| {
                                        "Source edit resulted in compile error.".to_string()
                                    });

                                panel.visual_edit_error =
                                    Some(format!("Edit Reverted: {}", error_msg));
                            }
                        }
                    }

                    cx.notify();

                    // If more keystrokes arrived while compiling, immediately start one compile for the latest text
                    if panel.pending_compile_source.is_some() {
                        panel.compile(cx);
                    }
                });
            }
        })
        .detach();
    }

    /// Updates the GpuiWorld's main document path and content.
    pub fn update_document_info(
        &mut self,
        path: Option<std::path::PathBuf>,
        _content: String,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        // We store the path update to be applied in the background compile task.
        // The content is handled by set_source which follows immediately.
        self.pending_compile_path = Some(path);
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

    fn sync_fonts_set_to_gpui(
        &mut self,
        used_fonts: std::collections::HashSet<typst::text::Font>,
        cx: &mut Context<Self>,
    ) {
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

    fn handle_link_click(&mut self, point: Point<Pixels>, cx: &mut Context<Self>) -> bool {
        for link in &self.last_hit_map.links {
            if link.bounds.contains(&point) {
                match &link.destination {
                    typst::model::Destination::Url(url) => {
                        let _ = gpui_kit::App::open_url(cx, url.as_str());
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
            let page_margin_px = gpui_kit::px(20.0 * self.zoom);
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

    pub fn diagnostics_at_offset(&self, offset: usize) -> Vec<&TextDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.range.start <= offset && offset <= d.range.end)
            .collect()
    }

    /// Adds a word to the user ignore list and re-runs spellcheck.
    pub fn ignore_word(&mut self, word: &str, cx: &mut Context<Self>) {
        self.dictionary_manager.lock().add_ignored_word(word);
        self.schedule_spellcheck(cx);
    }

    /// Asynchronously runs spell/grammar checks on GPUI's background executor.
    pub fn schedule_spellcheck(&mut self, cx: &mut Context<Self>) {
        if self.source.is_empty() {
            self.diagnostics.clear();
            cx.notify();
            return;
        }

        let text = Arc::new(self.source.clone());

        // Cancel any pending spellcheck task to debounce fast typing
        self.spellcheck_task = None;

        let dict_mgr_arc = self.dictionary_manager.clone();
        let cached_dict = self.cached_dictionary.clone();
        let cached_harper = self.cached_harper.clone();
        let handle = cx.weak_entity();

        self.spellcheck_task = Some(cx.spawn(|_view, spawned_async_cx: &mut AsyncApp| {
            let mut async_cx = spawned_async_cx.clone();
            async move {
                // 300ms debounce during continuous keystrokes
                async_cx
                    .background_executor()
                    .timer(Duration::from_millis(300))
                    .await;

                // Heavy AST prose extraction and dictionary checking on background thread
                let (diags, new_cached_dict, new_cached_harper) = async_cx
                    .background_executor()
                    .spawn(async move {
                        let source = typst_syntax::Source::detached(text.as_str());
                        let lang = detect_document_language(&source);

                        let mgr = dict_mgr_arc.lock();

                        // Load/cache dictionary if language changed
                        let dict_arc = match cached_dict {
                            Some((ref cached_lang, ref dict)) if cached_lang == &lang => {
                                Some(dict.clone())
                            }
                            _ => mgr.load_dictionary(&lang).ok(),
                        };

                        let harper_cache = if lang.starts_with("en") {
                            Some(cached_harper.unwrap_or_else(HarperCache::new_american))
                        } else {
                            cached_harper
                        };

                        let new_cache = dict_arc.as_ref().map(|d| (lang, d.clone()));
                        let results = check_source_with_harper(
                            &source,
                            dict_arc.as_deref(),
                            mgr.ignored_words(),
                            harper_cache.as_ref(),
                        );

                        (results, new_cache, harper_cache)
                    })
                    .await;

                // Update UI on main thread
                let _ = handle.update(&mut async_cx, |panel, cx| {
                    panel.diagnostics = diags;
                    if let Some(cache) = new_cached_dict {
                        panel.cached_dictionary = Some(cache);
                    }
                    if let Some(harper) = new_cached_harper {
                        panel.cached_harper = Some(harper);
                    }
                    cx.notify();
                });
            }
        }));
    }

    pub fn show_context_menu(
        &mut self,
        props: Option<typastry::wysiwyg::ElementProperties>,
        diagnostics: Vec<TextDiagnostic>,
        _point: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.inspected_element = props;
        self.active_menu_diagnostics = diagnostics;
        cx.notify();
    }
}

impl<W: TypstGpuiWorld> Render for PreviewPanel<W> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_focused = self.focus_handle.contains_focused(window, cx);

        gpui_kit::div()
            .id("preview-panel-root")
            .relative()
            .size_full()
            .bg(rgb(0x1a1a1a))
            .track_focus(&self.focus_handle)
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let current_text = this.source.clone();
                let current_sel = this.selection_range();

                let res = crate::canvas_input::CanvasInputHandler::handle_key_down(
                    event,
                    &current_text,
                    this.cursor_offset,
                    current_sel,
                    &this.last_hit_map,
                    cx,
                );

                if res.handled {
                    match res.action {
                        crate::canvas_input::CanvasAction::Undo => {
                            cx.emit(PreviewPanelEvent::UndoRequested);
                        }
                        crate::canvas_input::CanvasAction::Redo => {
                            cx.emit(PreviewPanelEvent::RedoRequested);
                        }
                        crate::canvas_input::CanvasAction::None => {
                            if let Some(ref new_text) = res.new_source {
                                this.set_source(new_text.clone(), window, cx);
                                cx.emit(PreviewPanelEvent::SourceChanged(new_text.to_string()));
                            }
                        }
                    }

                    if let Some(new_sel) = res.new_selection {
                        this.set_selection(new_sel, window, cx);
                    } else {
                        this.set_selection(res.new_cursor..res.new_cursor, window, cx);
                    }

                    cx.stop_propagation();
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
                        let text = this.source.clone();

                        match event.click_count {
                            1 => {
                                this.selection_anchor = Some(byte_offset);
                                this.cursor_offset = byte_offset;
                            }
                            2 => {
                                let range = find_word_boundaries(&text, byte_offset);
                                this.selection_anchor = Some(range.start);
                                this.cursor_offset = range.end;
                            }
                            3 => {
                                let range = find_paragraph_boundaries(&text, byte_offset);
                                this.selection_anchor = Some(range.start);
                                this.cursor_offset = range.end;
                            }
                            _ => {}
                        }
                        this.notify_cursor_moved(cx);
                    } else {
                        this.selection_anchor = None;
                        this.notify_cursor_moved(cx);
                        cx.notify();
                    }

                    window.focus(&this.focus_handle, cx);
                    cx.notify();
                    cx.stop_propagation();
                }),
            )
            // ==========================================
            // PILLAR 3: RIGHT CLICK INTROSPECTION TRIGGER
            // ==========================================
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                    if let Some(byte_offset) = this.offset_for_point(event.position) {
                        let text = this.source.clone();
                        let source = typst_syntax::Source::detached(&text);
                        let root = typst_syntax::LinkedNode::new(source.root());

                        // Query spell/grammar diagnostics at the clicked position
                        let active_diags: Vec<TextDiagnostic> = this
                            .diagnostics_at_offset(byte_offset)
                            .into_iter()
                            .cloned()
                            .collect();

                        let props = root
                            .leaf_at(byte_offset, typst_syntax::Side::Before)
                            .and_then(typastry::wysiwyg::inspect_element);

                        this.show_context_menu(props, active_diags, event.position, cx);
                    }
                    cx.stop_propagation();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _window, cx| {
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
            .on_scroll(
                cx.listener(|this, event: &gpui_kit::ScrollEvent, _win, cx| {
                    if event.modifiers.control || event.modifiers.platform {
                        let delta = event.delta.pixel_delta(gpui_kit::px(1.0)).y;
                        if delta > gpui_kit::px(0.) {
                            this.set_zoom(this.zoom + 0.1, cx);
                        } else if delta < gpui_kit::px(0.) {
                            this.set_zoom(this.zoom - 0.1, cx);
                        }
                    }
                }),
            )
            .child(
                // Stationary wrapper that hosts the scrollbar so it stays in place
                gpui_kit::div()
                    .id("preview-scroll-wrapper")
                    .relative()
                    .h_5_6()
                    .vertical_scrollbar(&self.scroll_handle)
                    .child(
                        // The actual scrolling container that handles the viewport and tracking
                        gpui_kit::div()
                            .id("preview-scroll-container")
                            .overflow_scroll()
                            .track_scroll(&self.scroll_handle)
                            .size_full()
                            .items_start()
                            .child(if let Some(doc) = &self.document {
                                TypstElement::new(
                                    doc.clone(),
                                    self.render_state.clone(),
                                    Some(self.cursor_offset),
                                    self.selection_range(),
                                    self.on_hit_map_updated_callback.clone(),
                                    self.cursor_visible,
                                    self.cached_span_resolver.clone(),
                                    self.diagnostics.clone(),
                                    self.cached_sources.clone(),
                                )
                                .with_zoom(self.zoom)
                                .into_any_element()
                            } else {
                                gpui_kit::div()
                                    .size_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(gpui_kit::rgb(0x666666))
                                    .child("No document compiled")
                                    .into_any_element()
                            }),
                    ),
            )
            // ==========================================
            // PILLAR 1: VISUAL EDITING ROLLBACK NOTIFIER
            // ==========================================
            .children(if let Some(ref err) = self.visual_edit_error {
                Some(
                    div()
                        .absolute()
                        .top_0()
                        .w_full()
                        .bg(rgb(0x7a2222))
                        .text_color(rgb(0xffffff))
                        .p_2()
                        .flex()
                        .justify_between()
                        .items_center()
                        .child(format!("⚠️ {}", err))
                        .child(
                            div()
                                .id("close-visual-edit-error")
                                .cursor(CursorStyle::PointingHand)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.visual_edit_error = None;
                                    cx.notify();
                                }))
                                .child("✕"),
                        ),
                )
            } else {
                None
            })
            // ==========================================
            // CONTEXT MENU OVERLAY (DIAGNOSTICS + INSPECTOR)
            // ==========================================
            .children(
                if !self.active_menu_diagnostics.is_empty() || self.inspected_element.is_some() {
                    Some(
                        div()
                            .id("wysiwyg-context-menu")
                            .absolute()
                            .top(px(20.0))
                            .right(px(20.0))
                            .w(px(280.0))
                            .bg(rgb(0x2d2d2d))
                            .border_1()
                            .border_color(rgb(0x444444))
                            .shadow_md()
                            .rounded_md()
                            .p_3()
                            .flex()
                            .flex_col()
                            .gap_2()
                            // 1. SPELL / GRAMMAR SUGGESTIONS SECTION
                            .children(self.active_menu_diagnostics.iter().map(|diag| {
                                let diag_clone = diag.clone();
                                div()
                                    .id("active-menu")
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_color(rgb(0xff6b6b))
                                            .font_semibold()
                                            .text_size(px(11.0))
                                            .child(diag.message.clone()),
                                    )
                                    .children(diag.suggestions.iter().take(4).map(|sugg| {
                                        let sugg_str = sugg.clone();
                                        let diag_for_click = diag_clone.clone();
                                        div()
                                            .id("suggestions")
                                            .cursor(CursorStyle::PointingHand)
                                            .bg(rgb(0x3a3a3a))
                                            .hover(|style| style.bg(rgb(0x4a4a4a)))
                                            .rounded_sm()
                                            .px_2()
                                            .py_1()
                                            .text_color(rgb(0xffffff))
                                            .text_size(px(12.0))
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                let edit = diag_for_click.to_text_edit(&sugg_str);
                                                let mut new_text = this.source.clone();
                                                new_text.replace_range(edit.range, &edit.new_text);
                                                this.set_source(new_text.clone(), window, cx);
                                                cx.emit(PreviewPanelEvent::SourceChanged(new_text));
                                                this.active_menu_diagnostics.clear();
                                                cx.notify();
                                            }))
                                            .child(format!("Replace with '{}'", sugg))
                                    }))
                                    .child(
                                        div()
                                            .id("flagged-text")
                                            .cursor(CursorStyle::PointingHand)
                                            .text_color(rgb(0x888888))
                                            .text_size(px(11.0))
                                            .px_2()
                                            .py_0p5()
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                let word = diag_clone.flagged_text.clone();
                                                this.ignore_word(&word, cx);
                                                this.active_menu_diagnostics.clear();
                                                cx.notify();
                                            }))
                                            .child("➕ Add to Ignore List"),
                                    )
                            })), // ... Rest of the Inspector context menu ...
                    )
                } else {
                    None
                },
            )
    }
}

// Support for gpui_component's Docking system
impl<W: TypstGpuiWorld> gpui_kit::base::dock::Panel for PreviewPanel<W> {
    fn panel_name(&self) -> &'static str {
        "PreviewPanel"
    }
}

impl<W: TypstGpuiWorld> component::dock::Panel for PreviewPanel<W> {
    fn title(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child("Preview")
    }
}

impl<W: TypstGpuiWorld> EventEmitter<gpui_kit::component::dock::PanelEvent> for PreviewPanel<W> {}

impl<W: TypstGpuiWorld> EventEmitter<PreviewPanelEvent> for PreviewPanel<W> {}

impl<W: TypstGpuiWorld> Focusable for PreviewPanel<W> {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

pub struct GpuiRegisteredFonts(pub std::collections::HashSet<u64>);
impl gpui_kit::Global for GpuiRegisteredFonts {}

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

    pub fn apply_visual_edit(
        &mut self,
        property_name: &str,
        property_value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = self.source.clone();

        let tx = typastry::wysiwyg::MutationTransaction {
            original_text: text.clone(),
            original_cursor: self.cursor_offset,
            original_selection: self.selection_range(),
        };
        self.pending_transaction = Some(tx);
        self.visual_edit_error = None;

        // --- FIX START ---
        // Use the start of the range of the *inspected element* as the target offset.
        // The inspected_element already identified the FuncCall/SetRule we want to modify.
        let target_element_offset = if let Some(ref inspected) = self.inspected_element {
            inspected.element_range.start
        } else {
            // Fallback: This case should ideally not be hit if the context menu is open.
            // Using cursor_offset as a fallback might still lead to the error message.
            self.cursor_offset
        };
        // --- FIX END ---

        if let Some((mutated_text, new_range)) = typastry::wysiwyg::apply_named_argument_edit(
            &text,
            target_element_offset, // Pass the corrected offset here
            property_name,
            property_value,
        ) {
            self.set_source(mutated_text.clone(), window, cx);
            cx.emit(PreviewPanelEvent::SourceChanged(mutated_text));
            self.set_selection(new_range, window, cx);

            if let Some(ref mut props) = self.inspected_element {
                props
                    .active_args
                    .insert(property_name.to_string(), property_value.to_string());
                props.available_properties.retain(|p| p != property_name);
            }
        } else {
            self.visual_edit_error =
                Some("Could not locate a customizable parent block in AST.".to_string());
            self.pending_transaction = None;
            cx.notify();
        }
    }

    /// Triggers the word-processor context menu on right-click over a component.
    pub fn show_wysiwyg_context_menu(
        &mut self,
        props: typastry::wysiwyg::ElementProperties,
        _point: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.inspected_element = Some(props);
        cx.notify();
    }
}

fn collect_fonts_from_frame_recursive_static(
    frame: &typst::layout::Frame,
    fonts_set: &mut std::collections::HashSet<typst::text::Font>,
) {
    for (_, item) in frame.items() {
        match item {
            typst::layout::FrameItem::Text(text) => {
                fonts_set.insert(text.font.font().clone());
            }
            typst::layout::FrameItem::Group(group) => {
                collect_fonts_from_frame_recursive_static(&group.frame, fonts_set);
            }
            _ => {}
        }
    }
}

fn collect_file_ids_from_frame_recursive(
    frame: &typst::layout::Frame,
    file_ids: &mut std::collections::HashSet<typst::syntax::FileId>,
) {
    for (_, item) in frame.items() {
        match item {
            typst::layout::FrameItem::Text(text_item) => {
                // Iterate over each glyph in the TextItem to get its span
                for glyph_instance in &text_item.glyphs {
                    let (span, _index) = glyph_instance.span; // glyph_instance.span is a (Span, u16) tuple
                    if let Some(id) = span.id() {
                        file_ids.insert(id);
                    }
                }
            }
            typst::layout::FrameItem::Image(_, _, span) => {
                if let Some(id) = span.id() {
                    file_ids.insert(id);
                }
            }
            typst::layout::FrameItem::Shape(_, span) => {
                if let Some(id) = span.id() {
                    file_ids.insert(id);
                }
            }
            typst::layout::FrameItem::Link(_, _) => { /* Links don't directly have a span ID for source mapping */
            }
            typst::layout::FrameItem::Tag(_) => { /* Tags don't directly have a span ID for source mapping */
            }
            typst::layout::FrameItem::Group(group_item) => {
                collect_file_ids_from_frame_recursive(&group_item.frame, file_ids);
            }
        }
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
