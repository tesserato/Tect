//! # Tect Language Server Backend
//!
//! Orchestrates documentation tooltips, navigation, formatting,
//! and semantic highlighting.

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
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(
                        SemanticTokensRegistrationOptions {
                            text_document_registration_options: TextDocumentRegistrationOptions {
                                document_selector: Some(vec![DocumentFilter {
                                    language: Some("tect".to_string()),
                                    scheme: Some("file".to_string()),
                                    pattern: None,
                                }]),
                            },
                            semantic_tokens_options: SemanticTokensOptions {
                                work_done_progress_options: WorkDoneProgressOptions {
                                    work_done_progress: None,
                                },
                                legend: SemanticTokensLegend {
                                    token_types: vec![
                                        SemanticTokenType::KEYWORD,   // 0
                                        SemanticTokenType::TYPE,      // 1
                                        SemanticTokenType::FUNCTION,  // 2
                                        SemanticTokenType::VARIABLE,  // 3
                                        SemanticTokenType::DECORATOR, // 4
                                    ],
                                    token_modifiers: vec![],
                                },
                                range: Some(false),
                                full: Some(SemanticTokensFullOptions::Bool(true)),
                            },
                            static_registration_options: StaticRegistrationOptions { id: None },
                        },
                    ),
                ),
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
                        let mut range = Self::span_to_range(content, meta.definition_span);
                        range.start.character = 0;
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

    async fn formatting(&self, p: DocumentFormattingParams) -> LspResult<Option<Vec<TextEdit>>> {
        let uri = &p.text_document.uri;
        if let Some(state) = self.document_state.get(uri) {
            let (content, _) = state.value();
            let mut formatted = String::new();

            if let Ok(pairs) = TectParser::parse(Rule::program, content) {
                for pair in pairs.flatten() {
                    match pair.as_rule() {
                        Rule::const_def | Rule::var_def | Rule::err_def | Rule::group_def => {
                            formatted.push_str("\n");
                            formatted.push_str(pair.as_str().trim());
                            formatted.push_str("\n");
                        }
                        Rule::func_def => {
                            formatted.push_str("\n");
                            let lines: Vec<&str> = pair.as_str().lines().collect();
                            if let Some(first) = lines.first() {
                                formatted.push_str(first.trim());
                                formatted.push_str("\n");
                                for line in lines.iter().skip(1) {
                                    formatted.push_str("    ");
                                    formatted.push_str(line.trim());
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
                let full_range = Range::new(Position::new(0, 0), Position::new(u32::MAX, u32::MAX));
                return Ok(Some(vec![TextEdit::new(
                    full_range,
                    formatted.trim().to_string() + "\n",
                )]));
            }
        }
        Ok(None)
    }

    async fn semantic_tokens_full(
        &self,
        p: SemanticTokensParams,
    ) -> LspResult<Option<SemanticTokensResult>> {
        let uri = &p.text_document.uri;
        let Some(state) = self.document_state.get(uri) else {
            return Ok(None);
        };
        let (content, structure) = state.value();

        let mut tokens = Vec::new();
        let (mut last_line, mut last_char) = (0, 0);

        if let Ok(pairs) = TectParser::parse(Rule::program, content) {
            for pair in pairs.flatten() {
                let token_type = match pair.as_rule() {
                    Rule::kw_constant
                    | Rule::kw_variable
                    | Rule::kw_error
                    | Rule::kw_group
                    | Rule::kw_function => Some(0),
                    Rule::ident => {
                        let word = pair.as_str();
                        if structure.artifacts.contains_key(word) {
                            Some(1)
                        } else if structure.catalog.contains_key(word) {
                            Some(2)
                        } else if structure.groups.contains_key(word) {
                            Some(4)
                        } else {
                            Some(3)
                        }
                    }
                    _ => None,
                };

                if let Some(idx) = token_type {
                    let span = pair.as_span();
                    let (line, col) = span.start_pos().line_col();
                    let line = line as u32 - 1;
                    let col = col as u32 - 1;

                    let delta_line = line - last_line;
                    let delta_start = if delta_line == 0 {
                        col - last_char
                    } else {
                        col
                    };

                    tokens.push(SemanticToken {
                        delta_line,
                        delta_start,
                        length: span.as_str().len() as u32,
                        token_type: idx,
                        token_modifiers_bitset: 0,
                    });

                    last_line = line;
                    last_char = col;
                }
            }
        }

        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: tokens,
        })))
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }
}

impl Backend {
    async fn process_change(&self, uri: Url, content: String) {
        let mut analyzer = TectAnalyzer::new();
        if let Ok(structure) = analyzer.analyze(&content) {
            self.document_state.insert(uri, (content, structure));
        } else {
            self.document_state
                .entry(uri)
                .and_modify(|(old_content, _)| *old_content = content);
        }
    }

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

    fn span_to_range(content: &str, span: Span) -> Range {
        let mut start_pos = Position::new(0, 0);
        let mut end_pos = Position::new(0, 0);
        let mut current_offset = 0;

        for (i, line) in content.lines().enumerate() {
            let line_len = line.len() + 1;
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
