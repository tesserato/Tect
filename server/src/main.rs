use pest::Parser;
use pest_derive::Parser;
use std::collections::HashMap;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

#[derive(Parser)]
#[grammar = "tect.pest"]
pub struct TectParser;

struct Backend {
    #[allow(dead_code)]
    client: Client,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                // Enable Hover
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                // Enable Semantic Highlighting
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
                                work_done_progress_options: Default::default(),
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
                            static_registration_options: StaticRegistrationOptions::default(),
                        },
                    ),
                ),
                ..Default::default()
            },
            ..Default::default()
        })
    }

async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;
    let Ok(content) = std::fs::read_to_string(uri.to_file_path().unwrap()) else { return Ok(None); };

    let mut symbols = HashMap::new();
    let mut docs = HashMap::new();

    if let Ok(pairs) = TectParser::parse(Rule::program, &content) {
        for pair in pairs.into_iter().next().unwrap().into_inner() {
            let rule = pair.as_rule();
            let mut current_docs = Vec::new();
            let mut name = String::new();

            // Walk inside data_def, error_def, etc.
            for inner in pair.into_inner() {
                match inner.as_rule() {
                    Rule::comment => {
                        let clean = inner.as_str().trim_start_matches('#').trim();
                        current_docs.push(clean.to_string());
                    }
                    Rule::ident if name.is_empty() => {
                        name = inner.as_str().to_string();
                        let type_idx = match rule {
                            Rule::data_def => 1,
                            Rule::error_def => 4,
                            Rule::func_def => 2,
                            _ => 3,
                        };
                        symbols.insert(name.clone(), type_idx);
                    }
                    _ => {}
                }
            }
            if !name.is_empty() && !current_docs.is_empty() {
                docs.insert(name, current_docs.join("\n"));
            }
        }
    }

        // Pass 2: Check if mouse is over a word
        let mut response = None;
        if let Ok(pairs) = TectParser::parse(Rule::program, &content) {
            for pair in pairs.flatten() {
                let (l, c) = pair.line_col();
                let (line, col) = (l as u32 - 1, c as u32 - 1);
                let len = pair.as_str().len() as u32;

                if pos.line == line && pos.character >= col && pos.character < (col + len) {
                    let word = pair.as_str();
                    let type_label = match symbols.get(word) {
                        Some(1) => "Data",
                        Some(2) => "Function",
                        Some(4) => "Error",
                        _ => "Variable",
                    };

                    let mut md = format!("**{}**: `{}`", type_label, word);
                    if let Some(doc) = docs.get(word) {
                        md.push_str("\n\n---\n\n");
                        md.push_str(doc);
                    }
                    response = Some(md);
                    break;
                }
            }
        }

        Ok(response.map(|value| Hover {
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
        let Ok(content) = std::fs::read_to_string(uri.to_file_path().unwrap()) else {
            return Ok(None);
        };
        let mut symbols = HashMap::new();
        let mut tokens = Vec::new();

        if let Ok(pairs) = TectParser::parse(Rule::program, &content) {
            // Learn defined names for coloring
            for pair in pairs.clone().into_iter().next().unwrap().into_inner() {
                let rule = pair.as_rule();
                for inner in pair.into_inner() {
                    if let Rule::ident = inner.as_rule() {
                        let type_idx = match rule {
                            Rule::data_def => 1,
                            Rule::error_def => 4,
                            Rule::func_def => 2,
                            _ => continue,
                        };
                        symbols.insert(inner.as_str().to_string(), type_idx);
                        break;
                    }
                }
            }

            let mut last_line = 0;
            let mut last_start = 0;
            for pair in pairs.flatten() {
                let token_type = match pair.as_rule() {
                    Rule::kw_data | Rule::kw_error | Rule::kw_func => Some(0),
                    Rule::ident => Some(*symbols.get(pair.as_str()).unwrap_or(&3)),
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
    let (service, socket) = LspService::new(|client| Backend { client });
    Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
        .serve(service)
        .await;
}
