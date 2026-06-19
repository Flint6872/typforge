use chrono::{Datelike, Timelike};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Instant, SystemTime};
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

// Cache containers to store parsed ASTs and raw file bytes with validation rate-limiting
struct CachedSource {
    source: Source,
    mtime: Option<SystemTime>,
    last_checked: Instant,
}

struct CachedFile {
    bytes: Bytes,
    mtime: Option<SystemTime>,
    last_checked: Instant,
}

/// A custom Typst world that provides fonts and files.
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

    // Thread-safe caches
    sources: Mutex<HashMap<FileId, CachedSource>>,
    files: Mutex<HashMap<FileId, CachedFile>>,
}

impl GpuiWorld {
    /// Creates a new `GpuiWorld` with a pre-initialized font book.
    /// The fonts must be provided by the host application.
    pub fn new(font_store: typst_kit::fonts::FontStore) -> Self {
        let default_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));

        GpuiWorld {
            library: LazyHash::new(Library::default()),
            font_store,
            root: default_root.clone(),
            current_document_physical_path: None,
            source_text: String::new(),
            main_file_id: FileId::new(RootedPath::new(
                VirtualRoot::Project,
                VirtualPath::new("/__main__.typ").expect("must be valid"),
            )),
            compiled_document: None,
            sources: Mutex::new(HashMap::new()),
            files: Mutex::new(HashMap::new()),
        }
    }

    /// Update the source code of the document.
    pub fn set_source(&mut self, source: String) {
        self.source_text = source;
    }

    pub fn set_main_document_info(&mut self, document_path: Option<PathBuf>, content: String) {
        let is_switching = document_path != self.current_document_physical_path;
        self.source_text = content;

        if is_switching {
            // Clear the caches entirely when loading/switching to a different document
            if let Ok(mut sources) = self.sources.lock() {
                sources.clear();
            }
            if let Ok(mut files) = self.files.lock() {
                files.clear();
            }
        }

        if let Some(path) = document_path {
            self.root = path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("/"))
                .to_path_buf();
            self.current_document_physical_path = Some(path.clone());

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
            self.root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
            self.current_document_physical_path = None;
            self.main_file_id = FileId::new(RootedPath::new(
                VirtualRoot::Project,
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
        if id.package().is_some() {
            return Err(FileError::NotFound(
                id.vpath().as_rooted_path().to_path_buf(),
            ));
        }
        Ok(self.root.join(id.vpath().as_rootless_path()))
    }
}

impl World for GpuiWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.font_store.book()
    }

    fn main(&self) -> FileId {
        self.main_file_id
    }

    /// Gets the content of a file, parsed as a Typst source.
    fn source(&self, id: FileId) -> FileResult<Source> {
        let mut sources = self.sources.lock().unwrap();

        // 1. Bypass check for main file
        if id == self.main_file_id {
            if let Some(cached) = sources.get(&id) {
                if cached.source.text() == self.source_text {
                    return Ok(cached.source.clone());
                }
            }
            let source = Source::new(id, self.source_text.clone());
            sources.insert(
                id,
                CachedSource {
                    source: source.clone(),
                    mtime: None,
                    last_checked: Instant::now(),
                },
            );
            return Ok(source);
        }

        let now = Instant::now();

        // 2. Serve IMMEDIATELY from cache if checked within the last 1000ms (avoids disk system calls)
        if let Some(cached) = sources.get(&id) {
            if now.duration_since(cached.last_checked) < std::time::Duration::from_millis(1000) {
                return Ok(cached.source.clone());
            }
        }

        // 3. Resolve path and get modification time (rate-limited)
        let path = self.resolve(id)?;
        let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();

        // 4. Serve from cache if modification time hasn't changed (Borrow separation fix)
        let mut cache_hit = false;
        if let Some(cached) = sources.get(&id) {
            if cached.mtime == mtime {
                cache_hit = true;
            }
        }

        if cache_hit {
            if let Some(entry) = sources.get_mut(&id) {
                entry.last_checked = now;
                return Ok(entry.source.clone());
            }
        }

        // 5. Cache Miss: Read from disk, parse, and store
        let text = std::fs::read_to_string(&path).map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => FileError::NotFound(path.clone()),
            std::io::ErrorKind::PermissionDenied => FileError::AccessDenied,
            _ => FileError::Other(None),
        })?;

        let source = Source::new(id, text);
        sources.insert(
            id,
            CachedSource {
                source: source.clone(),
                mtime,
                last_checked: now,
            },
        );
        Ok(source)
    }

    /// Reads a file's raw bytes.
    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.main_file_id {
            return Ok(Bytes::from_string(self.source_text.clone()));
        }

        let mut files = self.files.lock().unwrap();
        let now = Instant::now();

        // 1. Serve IMMEDIATELY from cache if checked within the last 1000ms
        if let Some(cached) = files.get(&id) {
            if now.duration_since(cached.last_checked) < std::time::Duration::from_millis(1000) {
                return Ok(cached.bytes.clone());
            }
        }

        // 2. Resolve path and get modification time
        let path = self.resolve(id)?;
        let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();

        // 3. Serve from cache if modification time hasn't changed (Borrow separation fix)
        let mut cache_hit = false;
        if let Some(cached) = files.get(&id) {
            if cached.mtime == mtime {
                cache_hit = true;
            }
        }

        if cache_hit {
            if let Some(entry) = files.get_mut(&id) {
                entry.last_checked = now;
                return Ok(entry.bytes.clone());
            }
        }

        // 4. Cache Miss: Read bytes from disk
        let data = std::fs::read(&path).map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => FileError::NotFound(path.clone()),
            std::io::ErrorKind::PermissionDenied => FileError::AccessDenied,
            _ => FileError::Other(None),
        })?;

        let bytes = Bytes::new(data);
        files.insert(
            id,
            CachedFile {
                bytes: bytes.clone(),
                mtime,
                last_checked: now,
            },
        );
        Ok(bytes)
    }

    fn font(&self, id: usize) -> Option<Font> {
        self.font_store.font(id)
    }

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
