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
                                        SemanticTokenType::KEYWORD,  // 0
                                        SemanticTokenType::TYPE,     // 1
                                        SemanticTokenType::FUNCTION, // 2
                                        SemanticTokenType::VARIABLE, // 3
                                        SemanticTokenType::ENUM,     // 4 (Errors)
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

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let Ok(content) = std::fs::read_to_string(uri.to_file_path().unwrap()) else {
            return Ok(None);
        };

        let mut hover_text = None;

        if let Ok(pairs) = TectParser::parse(Rule::program, &content) {
            // We use flatten to check every single token in the file
            for pair in pairs.flatten() {
                let (line_start, col_start) = pair.line_col();
                let line = (line_start - 1) as u32;
                let col_begin = (col_start - 1) as u32;
                let col_end = col_begin + pair.as_str().len() as u32;

                // Check if the mouse is over this specific token
                if pos.line == line && pos.character >= col_begin && pos.character < col_end {
                    hover_text = match pair.as_rule() {
                        Rule::ident => Some(format!("**Tect Identifier**: `{}`", pair.as_str())),
                        Rule::kw_data => {
                            Some("**Keyword**: `Data` - Defines a state structure.".into())
                        }
                        Rule::kw_error => {
                            Some("**Keyword**: `Error` - Defines a failure branch.".into())
                        }
                        Rule::kw_func => Some(
                            "**Keyword**: `Function` - Defines a logical transformation.".into(),
                        ),
                        _ => None,
                    };
                    if hover_text.is_some() {
                        break;
                    }
                }
            }
        }

        Ok(hover_text.map(|text| Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: text,
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
            // FIX: Iterate through the top-level definitions to find names
            // This ensures we know what 'Credentials' is even when used later.
            for pair in pairs.clone().into_iter().next().unwrap().into_inner() {
                match pair.as_rule() {
                    Rule::data_def => {
                        if let Some(id) = pair.into_inner().nth(1) {
                            symbols.insert(id.as_str().to_string(), 1);
                        }
                    }
                    Rule::error_def => {
                        if let Some(id) = pair.into_inner().nth(1) {
                            symbols.insert(id.as_str().to_string(), 4);
                        }
                    }
                    Rule::func_def => {
                        if let Some(id) = pair.into_inner().nth(1) {
                            symbols.insert(id.as_str().to_string(), 2);
                        }
                    }
                    _ => {}
                }
            }

            // Pass 2: Generate token deltas
            let mut last_line = 0;
            let mut last_start = 0;

            for pair in pairs.flatten() {
                let token_type = match pair.as_rule() {
                    Rule::kw_data | Rule::kw_error | Rule::kw_func => Some(0),
                    Rule::ident => Some(*symbols.get(pair.as_str()).unwrap_or(&3)),
                    _ => None,
                };

                if let Some(type_idx) = token_type {
                    let (line_raw, col_raw) = pair.line_col();
                    let line = (line_raw - 1) as u32;
                    let col = (col_raw - 1) as u32;
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
