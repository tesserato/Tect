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
                                legend: SemanticTokensLegend {
                                    token_types: vec![
                                        SemanticTokenType::KEYWORD,  // 0: Data, Function, Error
                                        SemanticTokenType::TYPE,     // 1: Data Names
                                        SemanticTokenType::FUNCTION, // 2: Function Names
                                        SemanticTokenType::VARIABLE, // 3: user_input, auth_res
                                        SemanticTokenType::ENUM,     // 4: Error Names
                                    ],
                                    token_modifiers: vec![],
                                },
                                full: Some(SemanticTokensFullOptions::Bool(true)),
                                ..Default::default()
                            },
                            static_registration_options: Default::default(),
                        },
                    ),
                ),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let Ok(content) = std::fs::read_to_string(uri.to_file_path().unwrap()) else {
            return Ok(None);
        };

        let mut tokens = Vec::new();
        let mut symbols = HashMap::new(); // word -> token_type_index

        // Pass 1: Build Symbol Table (Learn what is a Type vs Error)
        if let Ok(pairs) = TectParser::parse(Rule::program, &content) {
            for pair in pairs.flatten() {
                match pair.as_rule() {
                    Rule::data_def => {
                        symbols.insert(pair.into_inner().nth(1).unwrap().as_str().to_string(), 1);
                    }
                    Rule::error_def => {
                        symbols.insert(pair.into_inner().nth(1).unwrap().as_str().to_string(), 4);
                    }
                    Rule::func_def => {
                        symbols.insert(pair.into_inner().nth(1).unwrap().as_str().to_string(), 2);
                    }
                    _ => {}
                }
            }
        }

        // Pass 2: Generate Tokens
        let mut last_line = 0;
        let mut last_start = 0;

        if let Ok(pairs) = TectParser::parse(Rule::program, &content) {
            for pair in pairs.flatten() {
                let token_type = match pair.as_rule() {
                    Rule::kw_data | Rule::kw_error | Rule::kw_func => Some(0),
                    Rule::ident => {
                        let word = pair.as_str();
                        // Look up in symbol table, default to Variable (3)
                        Some(*symbols.get(word).unwrap_or(&3))
                    }
                    _ => None,
                };

                if let Some(type_idx) = token_type {
                    let (line, col) = pair.line_col();
                    let line = (line - 1) as u32;
                    let col = (col - 1) as u32;
                    let len = pair.as_str().len() as u32;

                    let delta_line = line - last_line;
                    let delta_start = if delta_line == 0 {
                        col - last_start
                    } else {
                        col
                    };

                    tokens.push(SemanticToken {
                        delta_line,
                        delta_start,
                        length: len,
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
