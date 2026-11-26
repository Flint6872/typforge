use chrono::{Datelike, Timelike};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Instant, SystemTime};
use typst::{
    Library, LibraryExt, World,
    diag::{FileError, FileResult},
    foundations::{Bytes, Datetime},
    syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot},
    text::{Font, FontBook},
    utils::LazyHash,
};

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

/// Headless Typst world implementation managing fonts, filesystem resolution, and source caching.
pub struct ProjectWorld {
    library: LazyHash<Library>,
    font_store: Arc<typst_kit::fonts::FontStore>,
    root: PathBuf,
    current_document_physical_path: Option<PathBuf>,
    source_text: String,
    main_file_id: FileId,

    sources: Mutex<HashMap<FileId, CachedSource>>,
    files: Mutex<HashMap<FileId, CachedFile>>,
}

impl ProjectWorld {
    pub fn new(font_store: Arc<typst_kit::fonts::FontStore>) -> Self {
        let default_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));

        Self {
            library: LazyHash::new(Library::default()),
            font_store,
            root: default_root,
            current_document_physical_path: None,
            source_text: String::new(),
            main_file_id: FileId::new(RootedPath::new(
                VirtualRoot::Project,
                VirtualPath::new("/__main__.typ").expect("must be valid"),
            )),
            sources: Mutex::new(HashMap::new()),
            files: Mutex::new(HashMap::new()),
        }
    }

    pub fn clone_shared(&self) -> Self {
        Self {
            library: self.library.clone(),
            font_store: Arc::clone(&self.font_store),
            root: self.root.clone(),
            current_document_physical_path: self.current_document_physical_path.clone(),
            source_text: self.source_text.clone(),
            main_file_id: self.main_file_id,
            sources: Mutex::new(HashMap::new()),
            files: Mutex::new(HashMap::new()),
        }
    }

    pub fn font_store(&self) -> &Arc<typst_kit::fonts::FontStore> {
        &self.font_store
    }

    pub fn set_source(&mut self, source: String) {
        self.source_text = source;
    }

    pub fn set_main_document_info(&mut self, document_path: Option<PathBuf>, content: String) {
        let is_switching = document_path != self.current_document_physical_path;
        self.source_text = content;

        if is_switching {
            self.sources.lock().clear();
            self.files.lock().clear();
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

    pub fn main_file_id(&self) -> FileId {
        self.main_file_id
    }

    pub fn source_text(&self) -> &str {
        &self.source_text
    }

    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    fn resolve(&self, id: FileId) -> FileResult<PathBuf> {
        match id.root() {
            VirtualRoot::Project => Ok(self.root.join(id.vpath().get_without_slash())),
            VirtualRoot::Package(spec) => {
                let rel = format!(
                    "{}/{}/{}/{}",
                    spec.namespace.as_str(),
                    spec.name.as_str(),
                    spec.version,
                    id.vpath().get_without_slash().to_string()
                );

                if let Some(data_pkgs) = typst_kit::packages::FsPackages::system_data() {
                    let path = data_pkgs.path().join(&rel);
                    if path.exists() {
                        return Ok(path);
                    }
                }

                if let Some(cache_pkgs) = typst_kit::packages::FsPackages::system_cache() {
                    let path = cache_pkgs.path().join(&rel);
                    if path.exists() {
                        return Ok(path);
                    }
                }

                Err(FileError::Package(typst::diag::PackageError::NotFound(
                    spec.clone(),
                )))
            }
        }
    }
}

impl World for ProjectWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.font_store.book()
    }

    fn main(&self) -> FileId {
        self.main_file_id
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        let mut sources = self.sources.lock();

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

        if let Some(cached) = sources.get(&id) {
            if now.duration_since(cached.last_checked) < std::time::Duration::from_millis(1000) {
                return Ok(cached.source.clone());
            }
        }

        let path = self.resolve(id)?;
        let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();

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

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.main_file_id {
            return Ok(Bytes::from_string(self.source_text.clone()));
        }

        let mut files = self.files.lock();
        let now = Instant::now();

        if let Some(cached) = files.get(&id) {
            if now.duration_since(cached.last_checked) < std::time::Duration::from_millis(1000) {
                return Ok(cached.bytes.clone());
            }
        }

        let path = self.resolve(id)?;
        let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();

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

impl typastry::IdeWorld for ProjectWorld {
    fn upcast(&self) -> &dyn World {
        self
    }
}
