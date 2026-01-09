//! # Tect Language Server Backend
//!
//! Orchestrates documentation tooltips, navigation, formatting, and advanced
//! IDE features like Rename, Outline, and Inlay Hints.

use crate::analyzer::{Rule, TectAnalyzer, TectParser};
use crate::engine::Flow;
use crate::models::{Cardinality, Function, Kind, ProgramStructure, Span, SymbolMetadata, Token};
use crate::vis_js::{self, VisData};
use dashmap::DashMap;
use pest::Parser;
use regex::Regex;
use serde_json::Value;
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
                    return Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
                        uri.clone(),
                        Self::span_to_range(content, meta.definition_span),
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
        if let Some(state) = self.document_state.get(uri) {
            let (content, structure) = state.value();
            let mut symbols = Vec::new();
            for kind in structure.artifacts.values() {
                if let Some(meta) = structure.symbol_table.get(&kind.uid()) {
                    symbols.push(self.make_symbol(
                        kind.name(),
                        SymbolKind::STRUCT,
                        content,
                        meta.definition_span,
                    ));
                }
            }
            for func in structure.catalog.values() {
                if let Some(meta) = structure.symbol_table.get(&func.uid) {
                    symbols.push(self.make_symbol(
                        &func.name,
                        SymbolKind::FUNCTION,
                        content,
                        meta.definition_span,
                    ));
                }
            }
            for group in structure.groups.values() {
                if let Some(meta) = structure.symbol_table.get(&group.uid) {
                    symbols.push(self.make_symbol(
                        &group.name,
                        SymbolKind::NAMESPACE,
                        content,
                        meta.definition_span,
                    ));
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
                    let edits = meta
                        .occurrences
                        .iter()
                        .map(|s| TextEdit::new(Self::span_to_range(content, *s), new_name.clone()))
                        .collect();
                    let mut map = std::collections::HashMap::new();
                    map.insert(uri.clone(), edits);
                    return Ok(Some(WorkspaceEdit {
                        changes: Some(map),
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
                    return Ok(Some(
                        meta.occurrences
                            .iter()
                            .map(|s| Location::new(uri.clone(), Self::span_to_range(content, *s)))
                            .collect(),
                    ));
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
        if let Some(state) = self.document_state.get(uri) {
            let (content, structure) = state.value();
            let mut hints = Vec::new();
            for step in &structure.flow {
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
            let (_, structure) = state.value();
            let mut flow = Flow::new(true);
            let graph = flow.simulate(structure);
            return Ok(vis_js::produce_vis_data(&graph));
        }
        Err(LspError::internal_error())
    }

    async fn process_change(&self, uri: Url, content: String) {
        let mut analyzer = TectAnalyzer::new();
        let structure = analyzer.analyze(&content);
        self.client
            .publish_diagnostics(uri.clone(), structure.diagnostics.clone(), None)
            .await;
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
        let inputs = f
            .consumes
            .iter()
            .map(Self::format_token)
            .collect::<Vec<_>>()
            .join(", ");
        let outputs = f
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
        format!("{} -> {}", inputs, outputs)
    }

    fn format_token(t: &Token) -> String {
        match t.cardinality {
            Cardinality::Collection => format!("[{}]", t.kind.name()),
            Cardinality::Unitary => t.kind.name().to_string(),
        }
    }
}

// --- Block-Based Formatter Implementation ---

struct Block {
    content: String,
    start_pos: usize,
    end_pos: usize,
}

pub fn format_tect_source(content: &str) -> Option<String> {
    let mut blocks = Vec::new();

    let parsed = match TectParser::parse(Rule::program, content) {
        Ok(mut p) => p.next().unwrap(),
        Err(_) => return None,
    };

    // 1. Blockify: Convert AST pairs into styled Blocks
    for pair in parsed.into_inner() {
        if pair.as_rule() == Rule::EOI {
            continue;
        }

        let span = pair.as_span();
        let formatted_content = match pair.as_rule() {
            Rule::func_def => format_function(pair),
            Rule::comment | Rule::flow_step => pair.as_str().trim().to_string(),
            _ => pair.as_str().trim().to_string(), // Constants, vars, etc.
        };

        if !formatted_content.is_empty() {
            blocks.push(Block {
                content: formatted_content,
                start_pos: span.start(),
                end_pos: span.end(),
            });
        }
    }

    if blocks.is_empty() {
        return Some(String::new());
    }

    // 2. Glue: Join blocks based on original source whitespace
    let mut result = String::new();
    result.push_str(&blocks[0].content);

    for i in 1..blocks.len() {
        let prev = &blocks[i - 1];
        let curr = &blocks[i];

        // Check original source between end of prev and start of curr
        let gap = &content[prev.end_pos..curr.start_pos];
        let newline_count = gap.chars().filter(|&c| c == '\n').count();

        // If gap has >= 2 newlines (a blank line), force \n\n.
        // Else use \n (this keeps comments attached to code if they were adjacent).
        let separator = if newline_count >= 2 { "\n\n" } else { "\n" };

        result.push_str(separator);
        result.push_str(&curr.content);
    }

    result.push('\n');
    Some(result)
}

fn format_function(pair: pest::iterators::Pair<Rule>) -> String {
    let mut inner = pair.clone().into_inner();
    let mut parts = Vec::new();
    let mut last_inner_pos = None;

    // 1. Extract Docs
    while let Some(p) = inner.peek() {
        let pos = p.as_span().start();
        if last_inner_pos == Some(pos) {
            break;
        }
        last_inner_pos = Some(pos);
        if p.as_rule() == Rule::doc_line {
            parts.push(inner.next().unwrap().as_str().trim().to_string());
        } else {
            break;
        }
    }

    // 2. Extract Header (Group, Function Keyword, Name, Inputs)
    let mut header = Vec::new();
    while let Some(p) = inner.peek() {
        if matches!(
            p.as_rule(),
            Rule::ident | Rule::kw_function | Rule::token_list
        ) {
            header.push(inner.next().unwrap().as_str().trim());
        } else {
            break;
        }
    }
    if !header.is_empty() {
        parts.push(header.join(" "));
    }

    // 3. Extract Outputs (Indented)
    if let Some(p) = inner.next() {
        for child in p.into_inner() {
            if child.as_rule() == Rule::output_line {
                let raw = child.as_str().trim();
                let symbol = if raw.starts_with('>') { ">" } else { "|" };
                let mut output_parts = child.into_inner();
                let tokens = output_parts.next().map(|t| t.as_str().trim()).unwrap_or("");
                parts.push(format!("    {} {}", symbol, tokens));
            }
        }
    }

    parts.join("\n")
}
