//! # Tect Language Server Backend
//!
//! Orchestrates documentation tooltips, navigation, formatting, and advanced
//! IDE features.
//!
//! Acts as the controller for the [Workspace], [Analyzer], and [Engine].

use crate::analyzer::Workspace;
use crate::engine::Flow;
use crate::formatter::format_tect_source;
use crate::models::{Cardinality, Function, Kind, ProgramStructure, SymbolMetadata, Token};
use crate::vis_js::{self, VisData};
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use tower_lsp::jsonrpc::Result as LspResult;
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
        self.process_change(p.text_document.uri, Some(p.text_document.text))
            .await;
    }

    async fn did_change(&self, p: DidChangeTextDocumentParams) {
        if let Some(c) = p.content_changes.into_iter().next() {
            self.process_change(p.text_document.uri, Some(c.text)).await;
        }
    }

    async fn hover(&self, p: HoverParams) -> LspResult<Option<Hover>> {
        let uri = &p.text_document_position_params.text_document.uri;
        let pos = p.text_document_position_params.position;

        // Acquire lock
        let mut ws = self.workspace.lock().unwrap();
        let path = uri.to_file_path().unwrap();

        // Ensure we have content for this file to check words
        let file_id = ws.source_manager.get_id(&path);
        ws.source_manager.load_file(file_id, None);

        let Some(content) = ws
            .source_manager
            .get_content(file_id)
            .map(|s: &str| s.to_string())
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
        let uri = &p.text_document_position_params.text_document.uri;
        let pos = p.text_document_position_params.position;

        let mut ws_guard = self.workspace.lock().unwrap();
        // Split borrow: access structure (read) and source_manager (mut)
        let Workspace {
            ref structure,
            ref mut source_manager,
        } = *ws_guard;

        let path = uri.to_file_path().unwrap();
        let file_id = source_manager.get_id(&path);
        source_manager.load_file(file_id, None);

        let Some(content) = source_manager
            .get_content(file_id)
            .map(|s: &str| s.to_string())
        else {
            return Ok(None);
        };

        if let Some((word, _)) = Self::get_word_at(&content, pos) {
            if let Some(meta) = self.find_meta(&word, structure) {
                if let Some(target_path) = source_manager.get_path(meta.definition_span.file_id) {
                    let target_uri = Url::from_file_path(target_path).unwrap();
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
        let uri = &p.text_document.uri;
        let path = uri.to_file_path().unwrap();

        let mut ws_guard = self.workspace.lock().unwrap();
        // Split borrow
        let Workspace {
            ref structure,
            ref mut source_manager,
        } = *ws_guard;

        let file_id = source_manager.get_id(&path);
        // Ensure loaded for range calculation
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
        let uri = &p.text_document_position.text_document.uri;
        let pos = p.text_document_position.position;
        let new_name = p.new_name;

        let mut ws_guard = self.workspace.lock().unwrap();
        // Split borrow
        let Workspace {
            ref structure,
            ref mut source_manager,
        } = *ws_guard;

        let path = uri.to_file_path().unwrap();
        let file_id = source_manager.get_id(&path);
        source_manager.load_file(file_id, None);

        let Some(content) = source_manager
            .get_content(file_id)
            .map(|s: &str| s.to_string())
        else {
            return Ok(None);
        };

        if let Some((word, _)) = Self::get_word_at(&content, pos) {
            if let Some(meta) = self.find_meta(&word, structure) {
                let mut changes = HashMap::new();

                for span in &meta.occurrences {
                    if let Some(target_path) = source_manager.get_path(span.file_id) {
                        let target_uri = Url::from_file_path(target_path).unwrap();
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
        let uri = &p.text_document_position.text_document.uri;
        let pos = p.text_document_position.position;

        let mut ws_guard = self.workspace.lock().unwrap();
        // Split borrow
        let Workspace {
            ref structure,
            ref mut source_manager,
        } = *ws_guard;

        let path = uri.to_file_path().unwrap();
        let file_id = source_manager.get_id(&path);
        source_manager.load_file(file_id, None);

        let Some(content) = source_manager
            .get_content(file_id)
            .map(|s: &str| s.to_string())
        else {
            return Ok(None);
        };

        if let Some((word, _)) = Self::get_word_at(&content, pos) {
            if let Some(meta) = self.find_meta(&word, structure) {
                let locs = meta
                    .occurrences
                    .iter()
                    .map(|span| {
                        let target_path = source_manager.get_path(span.file_id).unwrap();
                        let target_uri = Url::from_file_path(target_path).unwrap();
                        let range = source_manager.resolve_range(*span);
                        Location::new(target_uri, range)
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
        let uri = &p.text_document_position_params.text_document.uri;
        let pos = p.text_document_position_params.position;

        let mut ws = self.workspace.lock().unwrap();
        let path = uri.to_file_path().unwrap();
        let file_id = ws.source_manager.get_id(&path);
        ws.source_manager.load_file(file_id, None);

        let Some(content) = ws
            .source_manager
            .get_content(file_id)
            .map(|s: &str| s.to_string())
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
        let uri = &p.text_document.uri;
        let path = uri.to_file_path().unwrap();

        let mut ws_guard = self.workspace.lock().unwrap();
        // Split borrow
        let Workspace {
            ref structure,
            ref mut source_manager,
        } = *ws_guard;

        let file_id = source_manager.get_id(&path);
        // Resolve range relies on content loaded
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
        let path = p.text_document.uri.to_file_path().unwrap();
        let file_id = ws.source_manager.get_id(&path);

        // We ensure loaded because we might be formatting a file we haven't visited in graph yet
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
    pub async fn get_visual_graph(&self, _params: Value) -> LspResult<VisData> {
        // Graph generation runs on the current global structure
        let ws = self.workspace.lock().unwrap(); // Fixed: removed unused `mut`

        let mut flow = Flow::new(true);
        let graph = flow.simulate(&ws.structure);
        Ok(vis_js::produce_vis_data(&graph))
    }

    /// Triggers a full analysis of the dependency graph starting from the changed file.
    async fn process_change(&self, uri: Url, content: Option<String>) {
        let path = uri.to_file_path().unwrap();

        // Scope for the lock
        let (mut all_diagnostics, _) = {
            let mut ws_guard = self.workspace.lock().unwrap();

            // Analyze the project.
            // Design Decision: We treat the active file as the entry point for analysis.
            // This ensures the graph relevant to what the user is editing is updated.
            ws_guard.analyze(path, content);

            // Run Engine if no fatal errors are present in the static analysis
            if !ws_guard
                .structure
                .diagnostics
                .iter()
                .any(|d| d.severity == DiagnosticSeverity::ERROR)
            {
                let mut flow = Flow::new(true);
                let _graph = flow.simulate(&ws_guard.structure);
                ws_guard.structure.diagnostics.extend(flow.diagnostics);
            }

            // Resolve diagnostics from internal Context-Aware format to LSP format
            // Here we need to split the borrow to iterate structure.diagnostics while calling source_manager.resolve_range
            let Workspace {
                ref structure,
                ref mut source_manager,
            } = *ws_guard;

            let mut resolved = HashMap::new();
            let mut file_ids = Vec::new();

            for diag_ctx in &structure.diagnostics {
                if let Some(path) = source_manager.get_path(diag_ctx.file_id) {
                    let uri = Url::from_file_path(path).unwrap();
                    let range = if let Some(span) = diag_ctx.span {
                        source_manager.resolve_range(span)
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

                    resolved.entry(uri).or_insert_with(Vec::new).push(lsp_diag);
                    file_ids.push(diag_ctx.file_id);
                }
            }
            (resolved, file_ids)
        };

        // Ensure we publish empty diagnostics for the current file if it has none,
        // effectively clearing old errors from the editor view.
        all_diagnostics.entry(uri.clone()).or_default();

        for (furi, diags) in all_diagnostics {
            self.client.publish_diagnostics(furi, diags, None).await;
        }

        self.client
            .send_notification::<AnalysisFinished>(serde_json::json!({ "uri": uri.to_string() }))
            .await;
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
