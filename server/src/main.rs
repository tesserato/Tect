use pest::Parser;
use pest_derive::Parser;
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
                                        SemanticTokenType::TYPE,     // 1: Data names
                                        SemanticTokenType::FUNCTION, // 2: Function names
                                        SemanticTokenType::VARIABLE, // 3: Variables
                                        SemanticTokenType::ENUM,     // 4: Error names
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
        let mut last_line = 0;
        let mut last_start = 0;

        if let Ok(pairs) = TectParser::parse(Rule::program, &content) {
            for top_pair in pairs.into_iter().next().unwrap().into_inner() {
                // We walk through each definition and assign specific colors to children
                for part in top_pair.clone().into_inner().flatten() {
                    let token_type = match part.as_rule() {
                        Rule::ident => match top_pair.as_rule() {
                            Rule::data_def => Some(1),      // Data Name -> TYPE
                            Rule::error_def => Some(4),     // Error Name -> ENUM
                            Rule::func_def => Some(2),      // Function Name -> FUNCTION
                            Rule::instantiation => Some(3), // var_name -> VARIABLE
                            Rule::assignment => Some(3),    // res -> VARIABLE
                            _ => Some(1),
                        },
                        // The actual keywords "Data", "Error", "Function"
                        _ if ["Data", "Error", "Function"].contains(&part.as_str()) => Some(0),
                        _ => None,
                    };

                    if let Some(type_idx) = token_type {
                        let (line, col) = part.line_col();
                        let line = (line - 1) as u32;
                        let col = (col - 1) as u32;
                        let len = part.as_str().len() as u32;

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
