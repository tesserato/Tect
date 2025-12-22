use dashmap::DashMap;
use pest::Parser;
use pest_derive::Parser;
use std::collections::HashMap;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

#[derive(Parser)]
#[grammar = "tect.pest"]
pub struct TectParser;

#[derive(Debug, Clone)]
struct SymbolMetadata {
    kind: String,
    description: Option<String>,
}

struct Backend {
    #[allow(dead_code)]
    client: Client,
    document_map: DashMap<Url, String>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
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
                                        SemanticTokenType::KEYWORD,  // 0
                                        SemanticTokenType::TYPE,     // 1
                                        SemanticTokenType::FUNCTION, // 2
                                        SemanticTokenType::VARIABLE, // 3
                                        SemanticTokenType::ENUM,     // 4
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

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.document_map
            .insert(params.text_document.uri, params.text_document.text);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.into_iter().next() {
            self.document_map
                .insert(params.text_document.uri, change.text);
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let Some(content) = self.document_map.get(&uri) else {
            return Ok(None);
        };

        let mut symbol_table: HashMap<String, SymbolMetadata> = HashMap::new();

        if let Ok(pairs) = TectParser::parse(Rule::program, &content) {
            // RECURSIVE SCAN: Visit every single rule to find definitions
            for pair in pairs.flatten() {
                match pair.as_rule() {
                    Rule::data_def
                    | Rule::error_def
                    | Rule::func_def
                    | Rule::instantiation
                    | Rule::assignment => {
                        let rule = pair.as_rule();
                        let mut comments = Vec::new();
                        let mut name = String::new();

                        // Look at children to extract name and docs
                        for inner in pair.into_inner() {
                            match inner.as_rule() {
                                Rule::comment => comments.push(
                                    inner.as_str().trim_start_matches('#').trim().to_string(),
                                ),
                                Rule::ident if name.is_empty() => name = inner.as_str().to_string(),
                                _ => {}
                            }
                        }

                        if !name.is_empty() {
                            let kind = match rule {
                                Rule::data_def => "Data",
                                Rule::error_def => "Error",
                                Rule::func_def => "Function",
                                _ => "Variable",
                            };

                            // Don't overwrite existing definitions with weaker ones (e.g. usage)
                            symbol_table.entry(name).or_insert(SymbolMetadata {
                                kind: kind.to_string(),
                                description: if comments.is_empty() {
                                    None
                                } else {
                                    Some(comments.join("\n"))
                                },
                            });
                        }
                    }
                    _ => {}
                }
            }
        }

        let mut hover_content = None;
        if let Ok(pairs) = TectParser::parse(Rule::program, &content) {
            // Find the specific token under the cursor
            for pair in pairs.flatten() {
                let span = pair.as_span();
                let (start_line, start_col) = span.start_pos().line_col();
                let (end_line, end_col) = span.end_pos().line_col();

                // Convert to 0-indexed
                let s_line = (start_line - 1) as u32;
                let s_col = (start_col - 1) as u32;
                let e_line = (end_line - 1) as u32;
                let e_col = (end_col - 1) as u32;

                // Hit test
                let is_hit = if pos.line > s_line && pos.line < e_line {
                    true
                } else if pos.line == s_line && pos.line == e_line {
                    pos.character >= s_col && pos.character < e_col
                } else if pos.line == s_line {
                    pos.character >= s_col
                } else if pos.line == e_line {
                    pos.character < e_col
                } else {
                    false
                };

                if is_hit {
                    let word = pair.as_str();
                    let rule = pair.as_rule();

                    // 1. Check if it's a Keyword
                    let keyword_info = match rule {
                        Rule::kw_data => Some(("Keyword", "Defines a **Data** structure.")),
                        Rule::kw_error => Some(("Keyword", "Defines an **Error** branch.")),
                        Rule::kw_func => Some(("Keyword", "Defines a **Function** contract.")),
                        Rule::kw_match => Some(("Keyword", "Type-based branching logic.")),
                        Rule::kw_for => Some(("Keyword", "Range-based iterative loop.")),
                        Rule::kw_is => Some(("Keyword", "Type narrowing/check.")),
                        Rule::arrow | Rule::fat_arrow => {
                            Some(("Symbol", "Directional flow of data or logic."))
                        }
                        _ => None,
                    };

                    if let Some((kind, desc)) = keyword_info {
                        hover_content =
                            Some(format!("### {}: `{}`\n\n---\n\n{}", kind, word, desc));
                    } else if let Some(meta) = symbol_table.get(word) {
                        // 2. Check if it's a known identifier from our Symbol Table
                        let mut md = format!("### {}: `{}`", meta.kind, word);
                        if let Some(doc) = &meta.description {
                            md.push_str("\n\n---\n\n");
                            md.push_str(doc);
                        }
                        hover_content = Some(md);
                    }

                    if hover_content.is_some() {
                        break;
                    }
                }
            }
        }

        Ok(hover_content.map(|value| Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value,
            }),
            range: None,
        }))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let Some(content) = self.document_map.get(&uri) else {
            return Ok(None);
        };

        let mut symbols = HashMap::new();
        let mut tokens = Vec::new();

        if let Ok(pairs) = TectParser::parse(Rule::program, &content) {
            for pair in pairs.clone().flatten() {
                match pair.as_rule() {
                    Rule::data_def => {
                        if let Some(id) = pair.into_inner().find(|p| p.as_rule() == Rule::ident) {
                            symbols.insert(id.as_str().to_string(), 1);
                        }
                    }
                    Rule::error_def => {
                        if let Some(id) = pair.into_inner().find(|p| p.as_rule() == Rule::ident) {
                            symbols.insert(id.as_str().to_string(), 4);
                        }
                    }
                    Rule::func_def => {
                        if let Some(id) = pair.into_inner().find(|p| p.as_rule() == Rule::ident) {
                            symbols.insert(id.as_str().to_string(), 2);
                        }
                    }
                    _ => {}
                }
            }

            let mut last_line = 0;
            let mut last_start = 0;
            for pair in pairs.flatten() {
                let token_type = match pair.as_rule() {
                    Rule::kw_data
                    | Rule::kw_error
                    | Rule::kw_func
                    | Rule::kw_for
                    | Rule::kw_match
                    | Rule::kw_in
                    | Rule::kw_is
                    | Rule::kw_break => Some(0),
                    Rule::ident => Some(*symbols.get(pair.as_str()).unwrap_or(&3)),
                    Rule::number => Some(4),
                    _ => None,
                };

                if let Some(type_idx) = token_type {
                    let (l, c) = pair.line_col();
                    let (line, col) = (l as u32 - 1, c as u32 - 1);
                    let delta_line = line - last_line;
                    let delta_start = if delta_line == 0 {
                        col - last_start
                    } else {
                        col
                    };

                    tokens.push(SemanticToken {
                        delta_line,
                        delta_start,
                        length: pair.as_str().len() as u32,
                        token_type: type_idx,
                        token_modifiers_bitset: 0,
                    });
                    last_line = line;
                    last_start = col;
                }
            }
        }
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: tokens,
        })))
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let (service, socket) = LspService::new(|client| Backend {
        client,
        document_map: DashMap::new(),
    });
    Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
        .serve(service)
        .await;
}
