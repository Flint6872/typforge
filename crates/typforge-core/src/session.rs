use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditOrigin {
    Editor,
    Preview,
    Ribbon,
    Disk,
}

/// Tracks the canonical text, active file path, and monotonic revision
/// to prevent sync loops and stale debounce overwrites.
#[derive(Debug, Clone)]
pub struct DocumentSession {
    active_path: Option<PathBuf>,
    text: String,
    revision: u64,
    last_origin: EditOrigin,
}

impl Default for DocumentSession {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentSession {
    pub fn new() -> Self {
        Self {
            active_path: None,
            text: String::new(),
            revision: 0,
            last_origin: EditOrigin::Disk,
        }
    }

    /// Updates the session if path or text actually changed.
    /// Returns `Some(new_revision)` if changed, or `None` if identical (stops loopback).
    pub fn update(
        &mut self,
        path: Option<PathBuf>,
        text: String,
        origin: EditOrigin,
    ) -> Option<u64> {
        if self.active_path == path && self.text == text {
            return None;
        }

        self.active_path = path;
        self.text = text;
        self.last_origin = origin;
        self.revision += 1;
        Some(self.revision)
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn active_path(&self) -> Option<&PathBuf> {
        self.active_path.as_ref()
    }

    pub fn last_origin(&self) -> EditOrigin {
        self.last_origin
    }
}
