//! # Source Manager
//!
//! Acts as the Virtual File System (VFS) and source of truth for file contents.
//! Handles Url <-> FileId mapping, lazy loading of files, and resolving
//! byte-offset Spans to LSP Line/Column Ranges.

use crate::models::{FileId, Span};
use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicU32, Ordering};
use tower_lsp::lsp_types::{Position, Range, Url};

/// Manages source files, their contents, and their unique IDs.
pub struct SourceManager {
    file_map: HashMap<Url, FileId>,
    id_map: HashMap<FileId, Url>,
    contents: HashMap<FileId, String>,
    line_indices: HashMap<FileId, Vec<usize>>,
    next_id: AtomicU32,
}

impl Default for SourceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceManager {
    pub fn new() -> Self {
        Self {
            file_map: HashMap::new(),
            id_map: HashMap::new(),
            contents: HashMap::new(),
            line_indices: HashMap::new(),
            next_id: AtomicU32::new(1),
        }
    }

    /// Gets or creates a FileId for a URI.
    /// Does NOT read the content immediately.
    pub fn get_id(&mut self, uri: &Url) -> FileId {
        if let Some(&id) = self.file_map.get(uri) {
            id
        } else {
            let id = self.next_id.fetch_add(1, Ordering::SeqCst);
            self.file_map.insert(uri.clone(), id);
            self.id_map.insert(id, uri.clone());
            id
        }
    }

    pub fn get_uri(&self, id: FileId) -> Option<&Url> {
        self.id_map.get(&id)
    }

    pub fn get_content(&self, id: FileId) -> Option<&str> {
        self.contents.get(&id).map(|s| s.as_str())
    }

    /// Updates or loads file content.
    ///
    /// Logic:
    /// 1. If `explicit_content` is provided (e.g., from LSP didChange), update memory.
    /// 2. If no `explicit_content` but file is already in memory, do nothing (preserve unsaved changes).
    /// 3. If file not in memory, attempt to read from disk using the URI path.
    ///
    /// Returns true if content is available.
    pub fn load_file(&mut self, id: FileId, explicit_content: Option<String>) -> bool {
        // Case 1: Explicit update
        if let Some(content) = explicit_content {
            self.update_content(id, content);
            return true;
        }

        // Case 2: Already loaded
        if self.contents.contains_key(&id) {
            return true;
        }

        // Case 3: Load from disk
        if let Some(uri) = self.get_uri(id) {
            // Only file:// schemes can be read from disk
            if let Ok(path) = uri.to_file_path() {
                if let Ok(content) = fs::read_to_string(path) {
                    self.update_content(id, content);
                    return true;
                }
            }
        }
        false
    }

    fn update_content(&mut self, id: FileId, content: String) {
        let indices = self.compute_line_indices(&content);
        self.contents.insert(id, content);
        self.line_indices.insert(id, indices);
    }

    fn compute_line_indices(&self, content: &str) -> Vec<usize> {
        let mut indices = vec![0];
        for (i, b) in content.bytes().enumerate() {
            if b == b'\n' {
                indices.push(i + 1);
            }
        }
        indices
    }

    /// Converts a byte-offset Span into an LSP Range (Line/Col).
    /// Lazily loads the file if it is not currently in memory.
    pub fn resolve_range(&mut self, span: Span) -> Range {
        // Ensure content is loaded to calculate indices
        if !self.contents.contains_key(&span.file_id) {
            self.load_file(span.file_id, None);
        }

        let default = Range::default();
        let Some(indices) = self.line_indices.get(&span.file_id) else {
            return default;
        };
        let Some(content) = self.contents.get(&span.file_id) else {
            return default;
        };

        let find_pos = |offset: usize| -> Position {
            let line = match indices.binary_search(&offset) {
                Ok(i) => i,
                Err(i) => i - 1,
            };
            let line_start = indices[line];

            if offset < line_start {
                return Position::new(line as u32, 0);
            }

            let line_str = &content[line_start..offset];
            let col = line_str.chars().map(|c| c.len_utf16() as u32).sum();

            Position::new(line as u32, col)
        };

        let start = find_pos(span.start);
        let end_offset = span.end.min(content.len());
        let end = find_pos(end_offset);

        Range::new(start, end)
    }
}
