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
///
/// The `SourceManager` acts as a central repository for all source code files
/// encountered during the compilation or analysis process. It handles:
/// - Assigning unique `FileId`s to `Url`s.
/// - storing file contents in memory.
/// - Mapping byte offsets to line/column positions (for LSP).
pub struct SourceManager {
    /// Maps file URIs to internal FileIds.
    file_map: HashMap<Url, FileId>,
    /// Maps internal FileIds back to their URIs.
    id_map: HashMap<FileId, Url>,
    /// Stores the raw string content of files, keyed by FileId.
    contents: HashMap<FileId, String>,
    /// Stores line start indices for efficient line/column calculation.
    line_indices: HashMap<FileId, Vec<usize>>,
    /// Atomic counter for generating unique FileIds.
    next_id: AtomicU32,
}

impl Default for SourceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceManager {
    /// Creates a new, empty `SourceManager`.
    pub fn new() -> Self {
        Self {
            file_map: HashMap::new(),
            id_map: HashMap::new(),
            contents: HashMap::new(),
            line_indices: HashMap::new(),
            next_id: AtomicU32::new(1),
        }
    }

    /// Gets the `FileId` for a given URI, creating a new one if it doesn't exist.
    ///
    /// This method is thread-safe regarding ID generation, but note that the internal
    /// maps are not wrapped in concurrent locks in this struct definition (ownership is usually managed externally).
    ///
    /// # Note
    /// This does NOT read the content of the file immediately.
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

    /// Retrieves the URI corresponding to a `FileId`.
    pub fn get_uri(&self, id: FileId) -> Option<&Url> {
        self.id_map.get(&id)
    }

    /// Retrieves the cached content of a file, if available.
    pub fn get_content(&self, id: FileId) -> Option<&str> {
        self.contents.get(&id).map(|s| s.as_str())
    }

    /// Updates or loads file content into memory.
    ///
    /// # Logic
    /// 1. **Explicit Update**: If `explicit_content` is provided (e.g., from an LSP `didChange` event),
    ///    it updates the memory immediately.
    /// 2. **Already Loaded**: If no `explicit_content` is provided but the file is already in memory,
    ///    it does nothing (preserving potential unsaved changes in memory).
    /// 3. **Load from Disk**: If the file is not in memory and no explicit content is given,
    ///    it attempts to read from the disk using the URI path.
    ///
    /// # Returns
    /// `true` if content is successfully available (either updated, presisted, or loaded), `false` otherwise.
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

    /// Converts a byte-offset `Span` into an LSP `Range` (Line/Column).
    ///
    /// This function handles UTF-16 character width conversions as required by the LSP spec.
    /// It will lazily load the file content if it is not currently in memory to perform the line index calculation.
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
