//! # Tect Language Server Backend
//!
//! Orchestrates documentation tooltips, navigation, formatting, and advanced
//! IDE features.
//!
//! Acts as the controller for the [Workspace], [Analyzer], and [Engine].

use crate::analyzer::Workspace;
use crate::engine::Flow;
use crate::export::vis_js::VisData;
use crate::export::{dot, mermaid, tikz, vis_js};
use crate::formatter::format_tect_source;
use crate::models::{Cardinality, Function, Graph, Kind, ProgramStructure, SymbolMetadata, Token};
use regex::Regex;
use serde_json::Value;
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Mutex;
use tower_lsp::jsonrpc::{Error as LspError, Result as LspResult};
use tower_lsp::lsp_types::notification::Notification;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

/// Marker for the custom "Analysis Finished" notification.
pub enum AnalysisFinished {}
impl Notification for AnalysisFinished {
    type Params = Value;
    const METHOD: &'static str = "tect/analysisFinished";
}

/// The Backend holds the workspace state protected by a Mutex.
pub struct Backend {
    pub client: Client,
    pub workspace: Mutex<Workspace>,
    /// Tracks which files are currently open in the client.
    pub open_documents: Mutex<HashSet<Url>>,
    /// Caches the hash of the last successfully simulated graph per file.
    /// Used to suppress unnecessary UI updates.
    pub graph_cache: Mutex<HashMap<Url, u64>>,
}

impl Backend {
    fn compute_graph_hash(graph: &Graph) -> u64 {
        let mut s = DefaultHasher::new();
        graph.hash(&mut s);
        s.finish()
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> LspResult<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![" ".to_string()]),
                    ..Default::default()
                }),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec![" ".to_string()]),
                    ..Default::default()
                }),
                inlay_hint_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn did_open(&self, p: DidOpenTextDocumentParams) {
        {
            let mut docs = self.open_documents.lock().unwrap();
            docs.insert(p.text_document.uri.clone());
        }
        self.process_change(p.text_document.uri, Some(p.text_document.text))
            .await;
    }

    async fn did_close(&self, p: DidCloseTextDocumentParams) {
        {
            let mut docs = self.open_documents.lock().unwrap();
            docs.remove(&p.text_document.uri);
        }
    }

    async fn did_change(&self, p: DidChangeTextDocumentParams) {
        if let Some(c) = p.content_changes.into_iter().last() {
            self.process_change(p.text_document.uri, Some(c.text)).await;
        }
    }

    async fn hover(&self, p: HoverParams) -> LspResult<Option<Hover>> {
        let uri = p.text_document_position_params.text_document.uri;
        let pos = p.text_document_position_params.position;

        let mut ws = self.workspace.lock().unwrap();

        if ws.current_root.as_ref() != Some(&uri) {
            ws.analyze(uri.clone(), None);
        }

        let file_id = ws.source_manager.get_id(&uri);
        ws.source_manager.load_file(file_id, None);

        let Some(content) = ws
            .source_manager
            .get_content(file_id)
            .map(|s| s.to_string())
        else {
            return Ok(None);
        };

        if let Some((word, range)) = Self::get_word_at(&content, pos) {
            let kw_doc = match word.as_str() {
                "constant" => Some("Defines an immutable global architectural artifact."),
                "variable" => Some("Defines a mutable or stateful architectural artifact."),
                "error" => Some("Defines an architectural error state or exception branch."),
                "group" => {
                    Some("Organizes functions into logical architectural layers or modules.")
                }
                "function" => Some(
                    "Defines an architectural contract with specific inputs and result branches.",
                ),
                "import" => Some("Imports definitions from another Tect file."),
                _ => None,
            };

            if let Some(doc) = kw_doc {
                return Ok(Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: format!("### Keyword: `{}`\n\n---\n\n{}", word, doc),
                    }),
                    range: Some(range),
                }));
            }

            let markdown = if let Some(kind) = ws.structure.artifacts.get(&word) {
                format!(
                    "### {}: `{}`\n\n---\n\n{}",
                    match kind {
                        Kind::Constant(_) => "Constant",
                        Kind::Variable(_) => "Variable",
                        Kind::Error(_) => "Error",
                    },
                    word,
                    kind.docs().unwrap_or("*No documentation.*")
                )
            } else if let Some(f) = ws.structure.catalog.get(&word) {
                let group = f
                    .group
                    .as_ref()
                    .map(|g| format!("**Group**: `{}`\n\n", g.name))
                    .unwrap_or_default();
                let signature = format!("**Signature**: `{}`\n\n", Self::format_signature(f));
                format!(
                    "### Function: `{}`\n\n{}{}---\n\n{}",
                    word,
                    group,
                    signature,
                    f.documentation.as_deref().unwrap_or("*No documentation.*")
                )
            } else if let Some(g) = ws.structure.groups.get(&word) {
                format!(
                    "### Group: `{}`\n\n---\n\n{}",
                    word,
                    g.documentation
                        .as_deref()
                        .unwrap_or("*Architectural group.*")
                )
            } else {
                return Ok(None);
            };

            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: markdown,
                }),
                range: Some(range),
            }));
        }
        Ok(None)
    }

    async fn goto_definition(
        &self,
        p: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        let uri = p.text_document_position_params.text_document.uri;
        let pos = p.text_document_position_params.position;

        let mut ws_guard = self.workspace.lock().unwrap();

        if ws_guard.current_root.as_ref() != Some(&uri) {
            ws_guard.analyze(uri.clone(), None);
        }

        let Workspace {
            ref structure,
            ref mut source_manager,
            ..
        } = *ws_guard;

        let file_id = source_manager.get_id(&uri);
        source_manager.load_file(file_id, None);

        let Some(content) = source_manager.get_content(file_id).map(|s| s.to_string()) else {
            return Ok(None);
        };

        // Check for import path first
        if let Some(target_uri) = Self::check_import_at(&content, pos, &uri) {
            return Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
                target_uri,
                Range::new(Position::new(0, 0), Position::new(0, 0)),
            ))));
        }

        // Check for symbols
        if let Some((word, _)) = Self::get_word_at(&content, pos) {
            if let Some(meta) = self.find_meta(&word, structure) {
                if let Some(target_uri) = source_manager
                    .get_uri(meta.definition_span.file_id)
                    .cloned()
                {
                    let range = source_manager.resolve_range(meta.definition_span);
                    return Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
                        target_uri, range,
                    ))));
                }
            }
        }
        Ok(None)
    }

    async fn document_symbol(
        &self,
        p: DocumentSymbolParams,
    ) -> LspResult<Option<DocumentSymbolResponse>> {
        let uri = p.text_document.uri;

        let mut ws_guard = self.workspace.lock().unwrap();

        if ws_guard.current_root.as_ref() != Some(&uri) {
            ws_guard.analyze(uri.clone(), None);
        }

        let Workspace {
            ref structure,
            ref mut source_manager,
            ..
        } = *ws_guard;

        let file_id = source_manager.get_id(&uri);
        source_manager.load_file(file_id, None);

        let mut symbols = Vec::new();

        for kind in structure.artifacts.values() {
            if let Some(meta) = structure.symbol_table.get(&kind.uid()) {
                if meta.definition_span.file_id == file_id {
                    let range = source_manager.resolve_range(meta.definition_span);
                    symbols.push(self.make_symbol(kind.name(), SymbolKind::STRUCT, range));
                }
            }
        }
        for func in structure.catalog.values() {
            if let Some(meta) = structure.symbol_table.get(&func.uid) {
                if meta.definition_span.file_id == file_id {
                    let range = source_manager.resolve_range(meta.definition_span);
                    symbols.push(self.make_symbol(&func.name, SymbolKind::FUNCTION, range));
                }
            }
        }
        for group in structure.groups.values() {
            if let Some(meta) = structure.symbol_table.get(&group.uid) {
                if meta.definition_span.file_id == file_id {
                    let range = source_manager.resolve_range(meta.definition_span);
                    symbols.push(self.make_symbol(&group.name, SymbolKind::NAMESPACE, range));
                }
            }
        }
        return Ok(Some(DocumentSymbolResponse::Nested(symbols)));
    }

    async fn rename(&self, p: RenameParams) -> LspResult<Option<WorkspaceEdit>> {
        let uri = p.text_document_position.text_document.uri;
        let pos = p.text_document_position.position;
        let new_name = p.new_name;

        let mut ws_guard = self.workspace.lock().unwrap();

        if ws_guard.current_root.as_ref() != Some(&uri) {
            ws_guard.analyze(uri.clone(), None);
        }

        let Workspace {
            ref structure,
            ref mut source_manager,
            ..
        } = *ws_guard;

        let file_id = source_manager.get_id(&uri);
        source_manager.load_file(file_id, None);

        let Some(content) = source_manager.get_content(file_id).map(|s| s.to_string()) else {
            return Ok(None);
        };

        if let Some((word, _)) = Self::get_word_at(&content, pos) {
            if let Some(meta) = self.find_meta(&word, structure) {
                let mut changes = HashMap::new();

                for span in &meta.occurrences {
                    if let Some(target_uri) = source_manager.get_uri(span.file_id).cloned() {
                        let range = source_manager.resolve_range(*span);
                        changes
                            .entry(target_uri)
                            .or_insert_with(Vec::new)
                            .push(TextEdit::new(range, new_name.clone()));
                    }
                }

                return Ok(Some(WorkspaceEdit {
                    changes: Some(changes),
                    ..Default::default()
                }));
            }
        }
        Ok(None)
    }

    async fn references(&self, p: ReferenceParams) -> LspResult<Option<Vec<Location>>> {
        let uri = p.text_document_position.text_document.uri;
        let pos = p.text_document_position.position;

        let mut ws_guard = self.workspace.lock().unwrap();

        if ws_guard.current_root.as_ref() != Some(&uri) {
            ws_guard.analyze(uri.clone(), None);
        }

        let Workspace {
            ref structure,
            ref mut source_manager,
            ..
        } = *ws_guard;

        let file_id = source_manager.get_id(&uri);
        source_manager.load_file(file_id, None);

        let Some(content) = source_manager.get_content(file_id).map(|s| s.to_string()) else {
            return Ok(None);
        };

        if let Some((word, _)) = Self::get_word_at(&content, pos) {
            if let Some(meta) = self.find_meta(&word, structure) {
                let locs = meta
                    .occurrences
                    .iter()
                    .filter_map(|span| {
                        source_manager.get_uri(span.file_id).cloned().map(|uri| {
                            let range = source_manager.resolve_range(*span);
                            Location::new(uri, range)
                        })
                    })
                    .collect();

                return Ok(Some(locs));
            }
        }
        Ok(None)
    }

    async fn completion(&self, _p: CompletionParams) -> LspResult<Option<CompletionResponse>> {
        let ws = self.workspace.lock().unwrap();

        let mut items = Vec::new();
        for (name, kind) in &ws.structure.artifacts {
            items.push(CompletionItem {
                label: name.clone(),
                detail: Some("Artifact".into()),
                documentation: kind.docs().map(|d| Documentation::String(d.into())),
                kind: Some(CompletionItemKind::STRUCT),
                ..Default::default()
            });
        }
        for (name, func) in &ws.structure.catalog {
            items.push(CompletionItem {
                label: name.clone(),
                detail: Some("Function".into()),
                documentation: func.documentation.clone().map(Documentation::String),
                kind: Some(CompletionItemKind::FUNCTION),
                ..Default::default()
            });
        }
        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn signature_help(&self, p: SignatureHelpParams) -> LspResult<Option<SignatureHelp>> {
        let uri = p.text_document_position_params.text_document.uri;
        let pos = p.text_document_position_params.position;

        let mut ws = self.workspace.lock().unwrap();

        if ws.current_root.as_ref() != Some(&uri) {
            ws.analyze(uri.clone(), None);
        }

        let file_id = ws.source_manager.get_id(&uri);
        ws.source_manager.load_file(file_id, None);

        let Some(content) = ws
            .source_manager
            .get_content(file_id)
            .map(|s| s.to_string())
        else {
            return Ok(None);
        };

        if let Some((word, _)) = Self::get_word_at(&content, pos) {
            if let Some(f) = ws.structure.catalog.get(&word) {
                let sig = format!("{}: {}", f.name, Self::format_signature(f));
                return Ok(Some(SignatureHelp {
                    signatures: vec![SignatureInformation {
                        label: sig,
                        documentation: f.documentation.clone().map(Documentation::String),
                        parameters: None,
                        active_parameter: None,
                    }],
                    active_signature: Some(0),
                    active_parameter: Some(0),
                }));
            }
        }
        Ok(None)
    }

    async fn inlay_hint(&self, p: InlayHintParams) -> LspResult<Option<Vec<InlayHint>>> {
        let uri = p.text_document.uri;

        let mut ws_guard = self.workspace.lock().unwrap();

        if ws_guard.current_root.as_ref() != Some(&uri) {
            ws_guard.analyze(uri.clone(), None);
        }

        let Workspace {
            ref structure,
            ref mut source_manager,
            ..
        } = *ws_guard;

        let file_id = source_manager.get_id(&uri);
        source_manager.load_file(file_id, None);

        let mut hints = Vec::new();
        for step in &structure.flow {
            if step.span.file_id == file_id {
                if let Some(f) = structure.catalog.get(&step.function_name) {
                    let range = source_manager.resolve_range(step.span);
                    let signature = Self::format_signature(f);
                    let label = if let Some(ref g) = f.group {
                        format!("{} {}", g.name, signature)
                    } else {
                        signature
                    };

                    hints.push(InlayHint {
                        position: range.end,
                        label: InlayHintLabel::String(label),
                        kind: Some(InlayHintKind::TYPE),
                        padding_left: Some(true),
                        padding_right: None,
                        data: None,
                        tooltip: None,
                        text_edits: None,
                    });
                }
            }
        }
        Ok(Some(hints))
    }

    async fn formatting(&self, p: DocumentFormattingParams) -> LspResult<Option<Vec<TextEdit>>> {
        let mut ws = self.workspace.lock().unwrap();
        let uri = p.text_document.uri;
        let file_id = ws.source_manager.get_id(&uri);

        ws.source_manager.load_file(file_id, None);

        if let Some(content) = ws.source_manager.get_content(file_id) {
            if let Some(formatted) = format_tect_source(content) {
                let full_range = Range::new(Position::new(0, 0), Position::new(u32::MAX, u32::MAX));
                return Ok(Some(vec![TextEdit::new(full_range, formatted)]));
            }
        }
        Ok(None)
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }
}

impl Backend {
    /// Handler for `tect/getGraph`. Returns JSON data for Vis.js.
    pub async fn get_visual_graph(&self, params: Value) -> LspResult<VisData> {
        let uri_str = params
            .get("uri")
            .and_then(|v| v.as_str())
            .ok_or(LspError::invalid_params("Missing 'uri' parameter"))?;

        let uri =
            Url::parse(uri_str).map_err(|_| LspError::invalid_params("Invalid URI format"))?;

        let mut ws = self.workspace.lock().unwrap();
        ws.analyze(uri, None);

        let mut flow = Flow::new(true);
        let graph = flow.simulate(&ws.structure);

        Ok(vis_js::produce_vis_data(&graph))
    }

    /// Handler for `tect/exportGraph`. Returns the graph in various string formats.
    pub async fn get_export_content(&self, params: Value) -> LspResult<String> {
        let uri_str = params
            .get("uri")
            .and_then(|v| v.as_str())
            .ok_or(LspError::invalid_params("Missing 'uri' parameter"))?;

        let format = params
            .get("format")
            .and_then(|v| v.as_str())
            .ok_or(LspError::invalid_params("Missing 'format' parameter"))?;

        let uri =
            Url::parse(uri_str).map_err(|_| LspError::invalid_params("Invalid URI format"))?;

        let mut ws = self.workspace.lock().unwrap();
        ws.analyze(uri, None);

        let mut flow = Flow::new(true);
        let graph = flow.simulate(&ws.structure);

        match format {
            "dot" => Ok(dot::export(&graph)),
            "mermaid" => Ok(mermaid::export(&graph)),
            "tex" => Ok(tikz::export(&graph)),
            "json" => {
                Ok(serde_json::to_string_pretty(&graph).map_err(|e| LspError::internal_error())?)
            }
            _ => Err(LspError::invalid_params("Unknown format")),
        }
    }

    async fn process_change(&self, changed_uri: Url, content: Option<String>) {
        let open_docs: Vec<Url> = {
            let docs = self.open_documents.lock().unwrap();
            let mut list: Vec<Url> = docs.iter().cloned().collect();
            if let Some(pos) = list.iter().position(|u| u == &changed_uri) {
                list.remove(pos);
            }
            list.push(changed_uri.clone());
            list
        };

        let mut all_file_diagnostics: HashMap<Url, Vec<Diagnostic>> = HashMap::new();
        for uri in &open_docs {
            all_file_diagnostics.insert(uri.clone(), Vec::new());
        }

        let mut graph_to_notify: Option<Url> = None;

        {
            let mut ws_guard = self.workspace.lock().unwrap();

            if let Some(c) = content {
                let id = ws_guard.source_manager.get_id(&changed_uri);
                ws_guard.source_manager.load_file(id, Some(c));
            }

            for doc_uri in open_docs {
                ws_guard.analyze(doc_uri.clone(), None);

                let has_errors = ws_guard
                    .structure
                    .diagnostics
                    .iter()
                    .any(|d| d.severity == DiagnosticSeverity::ERROR);

                if !has_errors {
                    let mut flow = Flow::new(true);
                    let graph = flow.simulate(&ws_guard.structure);
                    ws_guard.structure.diagnostics.extend(flow.diagnostics);

                    // --- Differential Graph Check ---
                    // Only for the actively edited file do we care about notifying the graph
                    if doc_uri == changed_uri {
                        let new_hash = Self::compute_graph_hash(&graph);
                        let mut cache = self.graph_cache.lock().unwrap();
                        let last_hash = cache.get(&doc_uri).cloned().unwrap_or(0);

                        if new_hash != last_hash {
                            cache.insert(doc_uri.clone(), new_hash);
                            graph_to_notify = Some(doc_uri.clone());
                        }
                    }
                }

                let current_diagnostics = ws_guard.structure.diagnostics.clone();

                for diag_ctx in current_diagnostics {
                    if let Some(uri) = ws_guard.source_manager.get_uri(diag_ctx.file_id).cloned() {
                        let range = if let Some(span) = diag_ctx.span {
                            ws_guard.source_manager.resolve_range(span)
                        } else {
                            Range::default()
                        };

                        let lsp_diag = Diagnostic {
                            range,
                            severity: Some(diag_ctx.severity),
                            message: diag_ctx.message.clone(),
                            source: Some("tect".into()),
                            tags: if diag_ctx.tags.is_empty() {
                                None
                            } else {
                                Some(diag_ctx.tags.clone())
                            },
                            ..Default::default()
                        };

                        let entry = all_file_diagnostics.entry(uri).or_default();
                        if !entry.contains(&lsp_diag) {
                            entry.push(lsp_diag);
                        }
                    }
                }
            }
        }

        for (furi, diags) in all_file_diagnostics {
            self.client.publish_diagnostics(furi, diags, None).await;
        }

        if let Some(uri) = graph_to_notify {
            self.client
                .send_notification::<AnalysisFinished>(
                    serde_json::json!({ "uri": uri.to_string() }),
                )
                .await;
        }
    }

    fn find_meta<'a>(
        &self,
        word: &str,
        structure: &'a ProgramStructure,
    ) -> Option<&'a SymbolMetadata> {
        if let Some(kind) = structure.artifacts.get(word) {
            structure.symbol_table.get(&kind.uid())
        } else if let Some(f) = structure.catalog.get(word) {
            structure.symbol_table.get(&f.uid)
        } else if let Some(g) = structure.groups.get(word) {
            structure.symbol_table.get(&g.uid)
        } else {
            None
        }
    }

    fn make_symbol(&self, name: &str, kind: SymbolKind, range: Range) -> DocumentSymbol {
        #[allow(deprecated)]
        DocumentSymbol {
            name: name.to_string(),
            detail: None,
            kind,
            tags: None,
            range,
            selection_range: range,
            children: None,
            deprecated: None,
        }
    }

    fn check_import_at(content: &str, pos: Position, base_uri: &Url) -> Option<Url> {
        let line_str = content.lines().nth(pos.line as usize)?;

        // Regex to match: import "path"
        // Captures: 1 = quotes+path, 2 = path
        let re = Regex::new(r#"^\s*import\s+("([^"]+)")"#).ok()?;

        if let Some(cap) = re.captures(line_str) {
            let full_match = cap.get(1)?; // "path"
            let path_match = cap.get(2)?; // path

            let match_start = full_match.start();
            let match_end = full_match.end();

            // Calculate byte offset of cursor in line
            let mut char_offset = 0;
            let mut byte_offset = 0;
            for c in line_str.chars() {
                if char_offset == pos.character as usize {
                    break;
                }
                char_offset += 1;
                byte_offset += c.len_utf8();
            }

            // Check if cursor is strictly inside quotes
            if byte_offset > match_start && byte_offset < match_end {
                let rel_path = path_match.as_str();
                // Resolve relative to base_uri
                if let Ok(target) = base_uri.join(rel_path) {
                    return Some(target);
                }
            }
        }
        None
    }

    fn get_word_at(content: &str, pos: Position) -> Option<(String, Range)> {
        let line_str = content.lines().nth(pos.line as usize)?;
        let mut u16_off = 0;
        let mut b_off = 0;
        for c in line_str.chars() {
            if u16_off >= pos.character as usize {
                break;
            }
            u16_off += c.len_utf16();
            b_off += c.len_utf8();
        }
        let re = Regex::new(r"([a-zA-Z0-9_]+)").unwrap();
        for cap in re.find_iter(line_str) {
            if b_off >= cap.start() && b_off <= cap.end() {
                let s_u16 = line_str[..cap.start()].encode_utf16().count() as u32;
                let e_u16 = line_str[..cap.end()].encode_utf16().count() as u32;
                return Some((
                    cap.as_str().to_string(),
                    Range::new(
                        Position::new(pos.line, s_u16),
                        Position::new(pos.line, e_u16),
                    ),
                ));
            }
        }
        None
    }

    fn format_signature(f: &Function) -> String {
        let mut inputs = f
            .consumes
            .iter()
            .map(Self::format_token)
            .collect::<Vec<_>>()
            .join(", ");
        let mut outputs = f
            .produces
            .iter()
            .map(|branch| {
                branch
                    .iter()
                    .map(Self::format_token)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .collect::<Vec<_>>()
            .join(" | ");
        if inputs.is_empty() {
            inputs = "()".to_string();
        }
        if outputs.is_empty() {
            outputs = "()".to_string();
        }
        format!("{} -> {}", inputs, outputs)
    }

    fn format_token(t: &Token) -> String {
        match t.cardinality {
            Cardinality::Collection => format!("[{}]", t.kind.name()),
            Cardinality::Unitary => t.kind.name().to_string(),
        }
    }
}
