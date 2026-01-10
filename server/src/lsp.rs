//! # Tect Language Server Backend
//!
//! Orchestrates documentation tooltips, navigation, formatting, and advanced
//! IDE features like Rename, Outline, and Inlay Hints.

use crate::analyzer::TectAnalyzer;
use crate::engine::Flow;
use crate::formatter::format_tect_source;
use crate::models::{Cardinality, Function, Kind, ProgramStructure, Span, SymbolMetadata, Token};
use crate::vis_js::{self, VisData};
use dashmap::DashMap;
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
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

pub struct Backend {
    pub client: Client,
    pub document_state: DashMap<Url, (String, ProgramStructure)>,
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
        self.process_change(p.text_document.uri, p.text_document.text)
            .await;
    }

    async fn did_change(&self, p: DidChangeTextDocumentParams) {
        if let Some(c) = p.content_changes.into_iter().next() {
            self.process_change(p.text_document.uri, c.text).await;
        }
    }

    async fn hover(&self, p: HoverParams) -> LspResult<Option<Hover>> {
        let uri = &p.text_document_position_params.text_document.uri;
        let pos = p.text_document_position_params.position;

        if let Some(state) = self.document_state.get(uri) {
            let (content, structure) = state.value();
            if let Some((word, range)) = Self::get_word_at(content, pos) {
                let kw_doc = match word.as_str() {
                    "constant" => Some("Defines an immutable global architectural artifact."),
                    "variable" => Some("Defines a mutable or stateful architectural artifact."),
                    "error" => Some("Defines an architectural error state or exception branch."),
                    "group" => Some("Organizes functions into logical architectural layers or modules."),
                    "function" => Some("Defines an architectural contract with specific inputs and result branches."),
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

                let markdown = if let Some(kind) = structure.artifacts.get(&word) {
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
                } else if let Some(f) = structure.catalog.get(&word) {
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
                } else if let Some(g) = structure.groups.get(&word) {
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
        }
        Ok(None)
    }

    async fn goto_definition(
        &self,
        p: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        let uri = &p.text_document_position_params.text_document.uri;
        let pos = p.text_document_position_params.position;
        if let Some(state) = self.document_state.get(uri) {
            let (content, structure) = state.value();
            if let Some((word, _)) = Self::get_word_at(content, pos) {
                if let Some(meta) = self.find_meta(&word, structure) {
                    let target_uri = Url::from_file_path(&meta.source_file).unwrap();
                    // If target is same file, use content for range.
                    // If target is different, we can't calculate range easily without reading it.
                    // For now, if different file, we return 0,0 range.
                    let range = if target_uri == *uri {
                        Self::span_to_range(content, meta.definition_span)
                    } else {
                        // In a real implementation, we would cache file contents to calculate this
                        Range::default()
                    };

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

        if let Some(state) = self.document_state.get(uri) {
            let (content, structure) = state.value();
            let mut symbols = Vec::new();

            // Only show symbols defined in this file
            for kind in structure.artifacts.values() {
                if let Some(meta) = structure.symbol_table.get(&kind.uid()) {
                    if meta.source_file == path {
                        symbols.push(self.make_symbol(
                            kind.name(),
                            SymbolKind::STRUCT,
                            content,
                            meta.definition_span,
                        ));
                    }
                }
            }
            for func in structure.catalog.values() {
                if let Some(meta) = structure.symbol_table.get(&func.uid) {
                    if meta.source_file == path {
                        symbols.push(self.make_symbol(
                            &func.name,
                            SymbolKind::FUNCTION,
                            content,
                            meta.definition_span,
                        ));
                    }
                }
            }
            for group in structure.groups.values() {
                if let Some(meta) = structure.symbol_table.get(&group.uid) {
                    if meta.source_file == path {
                        symbols.push(self.make_symbol(
                            &group.name,
                            SymbolKind::NAMESPACE,
                            content,
                            meta.definition_span,
                        ));
                    }
                }
            }
            return Ok(Some(DocumentSymbolResponse::Nested(symbols)));
        }
        Ok(None)
    }

    async fn rename(&self, p: RenameParams) -> LspResult<Option<WorkspaceEdit>> {
        let uri = &p.text_document_position.text_document.uri;
        let pos = p.text_document_position.position;
        let new_name = p.new_name;
        if let Some(state) = self.document_state.get(uri) {
            let (content, structure) = state.value();
            if let Some((word, _)) = Self::get_word_at(content, pos) {
                if let Some(meta) = self.find_meta(&word, structure) {
                    let mut changes = HashMap::new();

                    for (file_path, span) in &meta.occurrences {
                        let file_uri = Url::from_file_path(file_path).unwrap();

                        // We need the content of that file to calc range.
                        // Limitation: rename only works in current file fully or blindly 0,0 elsewhere
                        // For the purpose of this implementation, we only support renaming in current file properly.
                        if file_uri == *uri {
                            let range = Self::span_to_range(content, *span);
                            changes
                                .entry(file_uri)
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
        }
        Ok(None)
    }

    async fn references(&self, p: ReferenceParams) -> LspResult<Option<Vec<Location>>> {
        let uri = &p.text_document_position.text_document.uri;
        let pos = p.text_document_position.position;
        if let Some(state) = self.document_state.get(uri) {
            let (content, structure) = state.value();
            if let Some((word, _)) = Self::get_word_at(content, pos) {
                if let Some(meta) = self.find_meta(&word, structure) {
                    let locs = meta
                        .occurrences
                        .iter()
                        .map(|(path, span)| {
                            let target_uri = Url::from_file_path(path).unwrap();
                            let range = if target_uri == *uri {
                                Self::span_to_range(content, *span)
                            } else {
                                Range::default() // Cannot resolve range without loading content
                            };
                            Location::new(target_uri, range)
                        })
                        .collect();

                    return Ok(Some(locs));
                }
            }
        }
        Ok(None)
    }

    async fn completion(&self, p: CompletionParams) -> LspResult<Option<CompletionResponse>> {
        let uri = &p.text_document_position.text_document.uri;
        if let Some(state) = self.document_state.get(uri) {
            let (_, structure) = state.value();
            let mut items = Vec::new();
            for (name, kind) in &structure.artifacts {
                items.push(CompletionItem {
                    label: name.clone(),
                    detail: Some("Artifact".into()),
                    documentation: kind.docs().map(|d| Documentation::String(d.into())),
                    kind: Some(CompletionItemKind::STRUCT),
                    ..Default::default()
                });
            }
            for (name, func) in &structure.catalog {
                items.push(CompletionItem {
                    label: name.clone(),
                    detail: Some("Function".into()),
                    documentation: func.documentation.clone().map(Documentation::String),
                    kind: Some(CompletionItemKind::FUNCTION),
                    ..Default::default()
                });
            }
            return Ok(Some(CompletionResponse::Array(items)));
        }
        Ok(None)
    }

    async fn signature_help(&self, p: SignatureHelpParams) -> LspResult<Option<SignatureHelp>> {
        let uri = &p.text_document_position_params.text_document.uri;
        let pos = p.text_document_position_params.position;
        if let Some(state) = self.document_state.get(uri) {
            let (content, structure) = state.value();
            if let Some((word, _)) = Self::get_word_at(content, pos) {
                if let Some(f) = structure.catalog.get(&word) {
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
        }
        Ok(None)
    }

    async fn inlay_hint(&self, p: InlayHintParams) -> LspResult<Option<Vec<InlayHint>>> {
        let uri = &p.text_document.uri;
        let path = uri.to_file_path().unwrap();

        if let Some(state) = self.document_state.get(uri) {
            let (content, structure) = state.value();
            let mut hints = Vec::new();
            for step in &structure.flow {
                // Only show hints for steps in the current file
                if step.source_file == path {
                    if let Some(f) = structure.catalog.get(&step.function_name) {
                        let range = Self::span_to_range(content, step.span);
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
            return Ok(Some(hints));
        }
        Ok(None)
    }

    async fn formatting(&self, p: DocumentFormattingParams) -> LspResult<Option<Vec<TextEdit>>> {
        let uri = &p.text_document.uri;
        if let Some(state) = self.document_state.get(uri) {
            let (content, _) = state.value();
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
    pub async fn get_visual_graph(&self, params: Value) -> LspResult<VisData> {
        let uri_str = params
            .get("uri")
            .and_then(|v| v.as_str())
            .ok_or_else(|| LspError::invalid_params("Missing 'uri' parameter"))?;
        let uri = Url::parse(uri_str).map_err(|_| LspError::invalid_params("Invalid URI"))?;

        if let Some(state) = self.document_state.get(&uri) {
            let (content, structure) = state.value();
            let mut flow = Flow::new(true);
            let graph = flow.simulate(structure, content); // Now passes content
            return Ok(vis_js::produce_vis_data(&graph));
        }
        Err(LspError::internal_error())
    }

    async fn process_change(&self, uri: Url, content: String) {
        let path = uri.to_file_path().unwrap();
        let mut analyzer = TectAnalyzer::new();
        let structure = analyzer.analyze(&content, path);

        // 1. Static Diagnostics
        let mut all_diagnostics = structure.diagnostics.clone();

        // 2. Engine Logic Diagnostics (Only if we have a generally valid structure)
        if !all_diagnostics
            .iter()
            .any(|(_, d)| d.severity == Some(DiagnosticSeverity::ERROR))
        {
            let mut flow = Flow::new(true);
            flow.simulate(&structure, &content);
            all_diagnostics.extend(flow.diagnostics);
        }

        // Group diagnostics by URI
        let mut diag_map: HashMap<Url, Vec<Diagnostic>> = HashMap::new();
        for (fpath, diag) in all_diagnostics {
            if let Ok(furi) = Url::from_file_path(fpath) {
                diag_map.entry(furi).or_default().push(diag);
            }
        }

        // Publish diagnostics for ALL affected files (including imports)
        // If a file is not in the map, we should publish empty diagnostics to clear old ones
        // In a real robust LSP, we'd track opened files. Here we publish what we found.
        // We also explicitly publish for current file to ensure it clears if valid.
        diag_map.entry(uri.clone()).or_default();

        for (furi, diags) in diag_map {
            self.client.publish_diagnostics(furi, diags, None).await;
        }

        self.document_state
            .insert(uri.clone(), (content, structure));

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

    fn make_symbol(
        &self,
        name: &str,
        kind: SymbolKind,
        content: &str,
        span: Span,
    ) -> DocumentSymbol {
        let range = Self::span_to_range(content, span);
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

    fn span_to_range(content: &str, span: Span) -> Range {
        let mut sp = Position::new(0, 0);
        let mut ep = Position::new(0, 0);
        let mut cb = 0;
        let mut l = 0;
        let mut c16 = 0;
        for c in content.chars() {
            if cb == span.start {
                sp = Position::new(l, c16);
            }
            if cb == span.end {
                ep = Position::new(l, c16);
                break;
            }
            if c == '\n' {
                l += 1;
                c16 = 0;
            } else {
                c16 += c.len_utf16() as u32;
            }
            cb += c.len_utf8();
        }
        Range::new(sp, ep)
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
