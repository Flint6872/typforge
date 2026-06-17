use chrono::{Datelike, Timelike};
use std::path::PathBuf;
use typst::{
    Library, LibraryExt, World,
    diag::{FileError, FileResult},
    foundations::{Bytes, Datetime},
    syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot},
    text::{Font, FontBook},
    utils::LazyHash,
};
use typst_layout::PagedDocument;

use typst_gpui::TypstGpuiWorld;

// A custom Typst world that provides fonts and files.

pub struct GpuiWorld {
    library: LazyHash<Library>,
    font_store: typst_kit::fonts::FontStore,
    /// The root directory for file resolution (used by #include, #image, etc).
    root: PathBuf,
    /// The physical path of the main document being compiled. `None` if new/unsaved.
    current_document_physical_path: Option<PathBuf>,
    /// The main source text that will be compiled. This is managed by the GPUI View.
    source_text: String,
    /// A virtual file ID for the source text.
    main_file_id: FileId,
    compiled_document: Option<std::sync::Arc<PagedDocument>>,
}

impl GpuiWorld {
    /// Creates a new `GpuiWorld` with a pre-initialized font book.
    /// The fonts must be provided by the host application.
    pub fn new(font_store: typst_kit::fonts::FontStore) -> Self {
        // Update signature
        // Initialize with a default root (e.g., current working directory)
        // and a generic virtual path for unsaved documents.
        let default_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));

        // Extract font_book and font_slots from the Fonts struct
        // let font_book = fonts_from_searcher.book;
        // let font_slots: Vec<std::sync::Arc<FontSlot>> = fonts_from_searcher
        //     .fonts
        //     .into_iter()
        //     .map(std::sync::Arc::new)
        //     .collect();

        // eprintln!(
        //     "GpuiWorld initialized with {} font slots and {} font families in book.",
        //     font_slots.len(),
        //     font_book.families().count()
        // );

        GpuiWorld {
            library: LazyHash::new(Library::default()),
            font_store, // Store the font_slots
            root: default_root.clone(),
            current_document_physical_path: None,
            source_text: String::new(),
            main_file_id: FileId::new(RootedPath::new(
                VirtualRoot::Project,
                VirtualPath::new("/__main__.typ").expect("must be valid"),
            )),
            compiled_document: None,
        }
    }

    /// Update the source code of the document.
    pub fn set_source(&mut self, source: String) {
        self.source_text = source;
    }

    pub fn set_main_document_info(&mut self, document_path: Option<PathBuf>, content: String) {
        self.source_text = content;

        if let Some(path) = document_path {
            // Set root to the parent directory of the document
            self.root = path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("/"))
                .to_path_buf();
            self.current_document_physical_path = Some(path.clone());

            // Create a virtual path for the document relative to its root.
            // e.g., if path is /home/user/doc.typ and root is /home/user/,
            // then relative_path is doc.typ, and vpath_str becomes /doc.typ.
            let relative_path = path.strip_prefix(&self.root).unwrap_or(&path);
            let mut vpath_str = relative_path.to_string_lossy().to_string();
            if !vpath_str.starts_with('/') {
                vpath_str = format!("/{}", vpath_str);
            }
            self.main_file_id = FileId::new(RootedPath::new(
                VirtualRoot::Project,
                VirtualPath::new(vpath_str).expect("must be valid"),
            ));
        } else {
            // If no path, it's a new/unsaved document. Use default root and generic virtual path.
            self.root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
            self.current_document_physical_path = None;
            self.main_file_id = FileId::new(RootedPath::new(
                VirtualRoot::Project, // 'None' is now a specific root type
                VirtualPath::new("/__main__.typ").expect("hardcoded path must be valid"),
            ));
        }
    }

    fn document(&self) -> Option<std::sync::Arc<PagedDocument>> {
        self.compiled_document.clone()
    }

    fn set_document(&mut self, doc: std::sync::Arc<PagedDocument>) {
        self.compiled_document = Some(doc);
    }

    /// Helper to resolve a Typst FileId to a physical path on disk.
    fn resolve(&self, id: FileId) -> FileResult<PathBuf> {
        // We currently only support project files (no package management yet).
        if id.package().is_some() {
            return Err(FileError::NotFound(
                id.vpath().as_rooted_path().to_path_buf(),
            ));
        }

        // Map the virtual path to the physical root.
        // as_rootless_path() converts "/path/to/file" to "path/to/file".
        Ok(self.root.join(id.vpath().as_rootless_path()))
    }
}

impl World for GpuiWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    /// Returns the font book, which contains metadata for all known fonts.
    fn book(&self) -> &LazyHash<FontBook> {
        &self.font_store.book()
    }

    // Returns the ID of the virtual main file that holds our source text.
    fn main(&self) -> FileId {
        self.main_file_id
    }

    /// Gets the content of a file, parsed as a Typst source.
    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main_file_id {
            return Ok(Source::new(id, self.source_text.clone()));
        }

        let path = self.resolve(id)?;
        let text = std::fs::read_to_string(&path).map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => FileError::NotFound(path),
            std::io::ErrorKind::PermissionDenied => FileError::AccessDenied,
            _ => FileError::Other(None),
        })?;

        Ok(Source::new(id, text))
    }

    /// Reads a file's raw bytes. For the MVP, only the main source is "read".
    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.main_file_id {
            return Ok(Bytes::from_string(self.source_text.clone()));
        }

        let path = self.resolve(id)?;
        let data = std::fs::read(&path).map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => FileError::NotFound(path),
            std::io::ErrorKind::PermissionDenied => FileError::AccessDenied,
            _ => FileError::Other(None),
        })?;

        Ok(Bytes::new(data))
    }

    /// Tries to access the font with the given identifier.
    fn font(&self, id: usize) -> Option<Font> {
        // Use the FontId's index to look up in our vector of loaded fonts.
        self.font_store.font(id)
    }

    /// Returns the current date and time.
    fn today(&self, _offset: Option<typst::foundations::Duration>) -> Option<Datetime> {
        let now = chrono::Local::now();
        Datetime::from_ymd_hms(
            now.year(),
            now.month() as u8,
            now.day() as u8,
            now.hour() as u8,
            now.minute() as u8,
            now.second() as u8,
        )
    }
}

impl TypstGpuiWorld for GpuiWorld {
    fn set_source(&mut self, source: String) {
        self.set_source(source);
    }

    fn set_main_document_info(&mut self, path: Option<std::path::PathBuf>, content: String) {
        self.set_main_document_info(path, content);
    }
}

// Implement IdeWorld for compiler-guided autocomplete and tooltip hover support
impl typforge_core::IdeWorld for GpuiWorld {
    fn upcast(&self) -> &dyn World {
        self
    }
}
