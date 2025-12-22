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

        let mut symbols: HashMap<String, SymbolMetadata> = HashMap::new();
        let mut comment_buffer: Vec<(usize, String)> = Vec::new(); // (line, content)

        if let Ok(pairs) = TectParser::parse(Rule::program, &content) {
            // PASS 1: Build Symbol Table and track comments
            for pair in pairs.clone().flatten() {
                let (line, _) = pair.line_col();
                match pair.as_rule() {
                    Rule::comment => {
                        let text = pair.as_str().trim_start_matches('#').trim().to_string();
                        comment_buffer.push((line, text));
                    }
                    Rule::data_def | Rule::error_def | Rule::func_def => {
                        let kind = match pair.as_rule() {
                            Rule::data_def => "Data",
                            Rule::error_def => "Error",
                            _ => "Function",
                        };
                        if let Some(id) = pair.into_inner().find(|p| p.as_rule() == Rule::ident) {
                            let name = id.as_str().to_string();

                            // Get comments immediately preceding this line
                            let mut docs = Vec::new();
                            let mut current_line = line - 1;
                            while let Some(idx) =
                                comment_buffer.iter().rposition(|(l, _)| *l == current_line)
                            {
                                docs.push(comment_buffer.remove(idx).1);
                                if current_line == 0 {
                                    break;
                                }
                                current_line -= 1;
                            }
                            docs.reverse();

                            symbols.insert(
                                name,
                                SymbolMetadata {
                                    kind: kind.into(),
                                    description: if docs.is_empty() {
                                        None
                                    } else {
                                        Some(docs.join("\n"))
                                    },
                                },
                            );
                        }
                    }
                    _ => {}
                }
            }

            // PASS 2: Find what's under the cursor
            for pair in pairs.flatten() {
                if !matches!(
                    pair.as_rule(),
                    Rule::ident
                        | Rule::kw_data
                        | Rule::kw_error
                        | Rule::kw_func
                        | Rule::kw_match
                        | Rule::kw_for
                ) {
                    continue;
                }
                let (l, c) = pair.line_col();
                let (line, col) = (l as u32 - 1, (c as u32 - 1));
                let len = pair.as_str().len() as u32;

                if pos.line == line && pos.character >= col && pos.character < (col + len) {
                    let word = pair.as_str();
                    let val = if let Some(meta) = symbols.get(word) {
                        format!(
                            "### {}: `{}`{}",
                            meta.kind,
                            word,
                            meta.description
                                .as_ref()
                                .map(|d| format!("\n\n---\n\n{}", d))
                                .unwrap_or_default()
                        )
                    } else {
                        match pair.as_rule() {
                            Rule::kw_data => "### Keyword: `Data`".into(),
                            Rule::kw_error => "### Keyword: `Error`".into(),
                            Rule::kw_func => "### Keyword: `Function`".into(),
                            Rule::kw_match => "### Keyword: `Match`".into(),
                            Rule::kw_for => "### Keyword: `For`".into(),
                            _ => format!("### Variable: `{}`", word),
                        }
                    };
                    return Ok(Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: val,
                        }),
                        range: None,
                    }));
                }
            }
        }
        Ok(None)
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
