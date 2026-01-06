//! # Tect Language Server Backend
//!
//! Orchestrates documentation tooltips, navigation, and formatting.
//! Implements standard LSP traits for communication with editors like VS Code.

use crate::analyzer::{Rule, TectAnalyzer, TectParser};
use crate::models::{Kind, ProgramStructure, Span};
use dashmap::DashMap;
use pest::Parser;
use regex::Regex;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

pub struct Backend {
    #[allow(dead_code)]
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

    /// Provides interactive tooltips for types and functions.
    async fn hover(&self, p: HoverParams) -> LspResult<Option<Hover>> {
        let uri = &p.text_document_position_params.text_document.uri;
        let pos = p.text_document_position_params.position;

        if let Some(state) = self.document_state.get(uri) {
            let (content, structure) = state.value();
            if let Some((word, range)) = Self::get_word_at(content, pos) {
                let markdown = if let Some(kind) = structure.artifacts.get(&word) {
                    let (lbl, docs) = match kind {
                        Kind::Constant(c) => ("Constant", &c.documentation),
                        Kind::Variable(v) => ("Variable", &v.documentation),
                        Kind::Error(e) => ("Error", &e.documentation),
                    };
                    format!(
                        "### {}: `{}`\n\n---\n\n{}",
                        lbl,
                        word,
                        docs.as_deref().unwrap_or("*No documentation.*")
                    )
                } else if let Some(f) = structure.catalog.get(&word) {
                    let group_line = f
                        .group
                        .as_ref()
                        .map(|g| format!("**Group**: `{}`\n\n", g.name))
                        .unwrap_or_default();
                    format!(
                        "### Function: `{}`\n\n{}---\n\n{}",
                        word,
                        group_line,
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

    /// Navigates the user to the definition of the symbol under the cursor.
    async fn goto_definition(
        &self,
        p: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        let uri = &p.text_document_position_params.text_document.uri;
        let pos = p.text_document_position_params.position;

        if let Some(state) = self.document_state.get(uri) {
            let (content, structure) = state.value();
            if let Some((word, _)) = Self::get_word_at(content, pos) {
                let uid = if let Some(k) = structure.artifacts.get(&word) {
                    Some(k.uid())
                } else if let Some(f) = structure.catalog.get(&word) {
                    Some(f.uid)
                } else if let Some(g) = structure.groups.get(&word) {
                    Some(g.uid)
                } else {
                    None
                };

                if let Some(id) = uid {
                    if let Some(meta) = structure.symbol_table.get(&id) {
                        let range = Self::span_to_range(content, meta.definition_span);
                        return Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
                            uri.clone(),
                            range,
                        ))));
                    }
                }
            }
        }
        Ok(None)
    }

    /// Reformats the document to comply with Tect style standards.
    async fn formatting(&self, p: DocumentFormattingParams) -> LspResult<Option<Vec<TextEdit>>> {
        let uri = &p.text_document.uri;
        if let Some(state) = self.document_state.get(uri) {
            let (content, _) = state.value();
            let mut formatted = String::new();

            if let Ok(mut pairs) = TectParser::parse(Rule::program, content) {
                let program = pairs.next().unwrap();
                for pair in program.into_inner() {
                    match pair.as_rule() {
                        Rule::const_def | Rule::var_def | Rule::err_def | Rule::group_def => {
                            formatted.push_str("\n");
                            formatted.push_str(pair.as_str().trim());
                            formatted.push_str("\n");
                        }
                        Rule::func_def => {
                            formatted.push_str("\n");
                            let mut inner = pair.clone().into_inner();

                            // 1. Process Documentation
                            while let Some(p) = inner.peek() {
                                if p.as_rule() == Rule::doc_line {
                                    formatted.push_str(inner.next().unwrap().as_str().trim());
                                    formatted.push_str("\n");
                                } else {
                                    break;
                                }
                            }

                            // 2. Process Group, keyword, and Name
                            let mut header_parts = Vec::new();
                            while let Some(p) = inner.peek() {
                                if p.as_rule() == Rule::ident || p.as_rule() == Rule::kw_function {
                                    header_parts.push(inner.next().unwrap().as_str().trim());
                                } else {
                                    break;
                                }
                            }
                            formatted.push_str(&header_parts.join(" "));

                            // 3. Process Inputs (no parens)
                            if let Some(p) = inner.peek() {
                                if p.as_rule() == Rule::token_list {
                                    formatted.push_str(" ");
                                    formatted.push_str(inner.next().unwrap().as_str().trim());
                                }
                            }
                            formatted.push_str("\n");

                            // 4. Process Outputs
                            if let Some(p) = inner.next() {
                                for out in p.into_inner() {
                                    formatted.push_str("    ");
                                    formatted.push_str(out.as_str().trim());
                                    formatted.push_str("\n");
                                }
                            }
                        }
                        Rule::flow_step => {
                            formatted.push_str(pair.as_str().trim());
                            formatted.push_str("\n");
                        }
                        Rule::comment => {
                            formatted.push_str(pair.as_str().trim());
                            formatted.push_str("\n");
                        }
                        _ => {}
                    }
                }
            }

            if !formatted.is_empty() {
                // Return a single edit that replaces the entire document.
                let full_range = Range::new(Position::new(0, 0), Position::new(u32::MAX, u32::MAX));
                return Ok(Some(vec![TextEdit::new(
                    full_range,
                    formatted.trim_start().to_string(),
                )]));
            }
        }
        Ok(None)
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }
}

impl Backend {
    /// Re-analyzes the document and updates the internal state.
    async fn process_change(&self, uri: Url, content: String) {
        let mut analyzer = TectAnalyzer::new();
        if let Ok(structure) = analyzer.analyze(&content) {
            self.document_state.insert(uri, (content, structure));
        } else {
            // Keep content but skip IR update if syntax is invalid.
            self.document_state
                .entry(uri)
                .and_modify(|(old_content, _)| *old_content = content);
        }
    }

    /// Identifies the word and range at a specific cursor position.
    fn get_word_at(content: &str, pos: Position) -> Option<(String, Range)> {
        let lines: Vec<&str> = content.lines().collect();
        let line = lines.get(pos.line as usize)?;
        let re = Regex::new(r"([a-zA-Z0-9_]+)").unwrap();
        for cap in re.find_iter(line) {
            if pos.character >= cap.start() as u32 && pos.character <= cap.end() as u32 {
                let range = Range::new(
                    Position::new(pos.line, cap.start() as u32),
                    Position::new(pos.line, cap.end() as u32),
                );
                return Some((cap.as_str().to_string(), range));
            }
        }
        None
    }

    /// Translates a byte-offset Span into a line/character Range.
    fn span_to_range(content: &str, span: Span) -> Range {
        let mut start_pos = Position::new(0, 0);
        let mut end_pos = Position::new(0, 0);
        let mut current_offset = 0;

        for (i, line) in content.lines().enumerate() {
            let line_len = line.len() + 1; // +1 for normalized \n
            if current_offset <= span.start && span.start < current_offset + line_len {
                start_pos = Position::new(i as u32, (span.start - current_offset) as u32);
            }
            if current_offset <= span.end && span.end <= current_offset + line_len {
                end_pos = Position::new(i as u32, (span.end - current_offset) as u32);
                break;
            }
            current_offset += line_len;
        }
        Range::new(start_pos, end_pos)
    }
}
