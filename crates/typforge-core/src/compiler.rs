use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use typst::World;
use typst::diag::SourceDiagnostic;
use typst::layout::{Frame, FrameItem};
use typst::syntax::{FileId, Source};
use typst::text::Font;
use typst_layout::PagedDocument;

use crate::world::ProjectWorld;

/// Compilation request sent to the background compiler worker.
#[derive(Debug, Clone)]
pub struct CompileRequest {
    pub revision: u64,
    pub path: Option<PathBuf>,
    pub source: String,
}

/// Headless compilation output delivered to the host application.
#[derive(Clone)]
pub struct CompileOutput {
    pub revision: u64,
    pub document: Option<Arc<PagedDocument>>,
    pub warnings: Vec<SourceDiagnostic>,
    pub errors: Vec<SourceDiagnostic>,
    pub used_fonts: HashSet<Font>,
    pub sources: HashMap<FileId, Arc<Source>>,
}

impl CompileOutput {
    pub fn is_ok(&self) -> bool {
        self.document.is_some() && self.errors.is_empty()
    }
}

/// Headless compiler manager operating on a dedicated background thread.
/// Coalesces rapid keystrokes automatically by draining pending requests.
pub struct CompilerHost {
    tx_request: Sender<CompileRequest>,
    rx_output: Receiver<CompileOutput>,
    current_revision: u64,
}

impl CompilerHost {
    pub fn new(mut world: ProjectWorld) -> Self {
        let (tx_req, rx_req) = channel::<CompileRequest>();
        let (tx_out, rx_out) = channel::<CompileOutput>();

        thread::spawn(move || {
            while let Ok(mut req) = rx_req.recv() {
                // Drain any pending requests to always compile the LATEST revision (coalescing)
                while let Ok(next_req) = rx_req.try_recv() {
                    req = next_req;
                }

                let output = perform_compile(&mut world, req);
                if tx_out.send(output).is_err() {
                    break; // Receiver dropped, worker thread terminates cleanly
                }
            }
        });

        Self {
            tx_request: tx_req,
            rx_output: rx_out,
            current_revision: 0,
        }
    }

    /// Submit a new document revision for background compilation.
    pub fn request_compile(&mut self, path: Option<PathBuf>, source: String) -> u64 {
        self.current_revision += 1;
        let rev = self.current_revision;
        let _ = self.tx_request.send(CompileRequest {
            revision: rev,
            path,
            source,
        });
        rev
    }

    /// Non-blocking check for finished compilation outputs.
    pub fn try_recv(&self) -> Option<CompileOutput> {
        self.rx_output.try_recv().ok()
    }

    pub fn current_revision(&self) -> u64 {
        self.current_revision
    }
}

fn perform_compile(world: &mut ProjectWorld, req: CompileRequest) -> CompileOutput {
    world.set_main_document_info(req.path, req.source);

    let warned = typst::compile::<PagedDocument>(world);
    let warnings: Vec<_> = warned.warnings.into_iter().collect();

    match warned.output {
        Ok(doc) => {
            let doc_arc = Arc::new(doc);
            let mut used_fonts = HashSet::new();
            let mut referenced_ids = HashSet::new();

            for page in doc_arc.pages() {
                // This will now work
                collect_file_ids_from_frame(&page.frame, &mut referenced_ids);
                collect_fonts_from_frame(&page.frame, &mut used_fonts);
            }

            let mut sources_map = HashMap::new();
            let main_id = world.main_file_id();
            if let Ok(main_source) = world.source(main_id) {
                sources_map.insert(main_id, Arc::new(main_source));
            }
            for id in referenced_ids {
                if !sources_map.contains_key(&id) {
                    if let Ok(src) = world.source(id) {
                        sources_map.insert(id, Arc::new(src));
                    }
                }
            }

            CompileOutput {
                revision: req.revision,
                document: Some(doc_arc), // This will now correctly be Arc<PagedDocument>
                warnings,
                errors: Vec::new(),
                used_fonts,
                sources: sources_map,
            }
        }
        Err(errors) => CompileOutput {
            revision: req.revision,
            document: None,
            warnings,
            errors: errors.into_iter().collect(),
            used_fonts: HashSet::new(),
            sources: HashMap::new(),
        },
    }
}

fn collect_fonts_from_frame(frame: &Frame, fonts_set: &mut HashSet<Font>) {
    for (_, item) in frame.items() {
        match item {
            FrameItem::Text(text) => {
                fonts_set.insert(text.font.font().clone());
            }
            FrameItem::Group(group) => {
                collect_fonts_from_frame(&group.frame, fonts_set);
            }
            _ => {}
        }
    }
}

fn collect_file_ids_from_frame(frame: &Frame, file_ids: &mut HashSet<FileId>) {
    for (_, item) in frame.items() {
        match item {
            FrameItem::Text(text_item) => {
                for glyph_instance in &text_item.glyphs {
                    let (span, _) = glyph_instance.span;
                    if let Some(id) = span.id() {
                        file_ids.insert(id);
                    }
                }
            }
            FrameItem::Image(_, _, span) => {
                if let Some(id) = span.id() {
                    file_ids.insert(id);
                }
            }
            FrameItem::Shape(_, span) => {
                if let Some(id) = span.id() {
                    file_ids.insert(id);
                }
            }
            FrameItem::Group(group_item) => {
                collect_file_ids_from_frame(&group_item.frame, file_ids);
            }
            _ => {}
        }
    }
}
