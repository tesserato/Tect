use pest::Parser;
use pest_derive::Parser;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

#[derive(Parser)]
#[grammar = "src/tect.pest"]
pub struct TectParser;

// 1. Define our "Color Palette"
lazy_static::lazy_static! {
    static ref TOKEN_TYPES: Vec<SemanticTokenType> = vec![
        SemanticTokenType::KEYWORD,   // 0: Data, Function, Error
        SemanticTokenType::TYPE,      // 1: Data names
        SemanticTokenType::FUNCTION,  // 2: Function names
        SemanticTokenType::VARIABLE,  // 3: creds_john
        SemanticTokenType::ENUM,      // 4: Error names
    ];
}

struct Backend {
    client: Client,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                // Tell VS Code we support Semantic Tokens
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(
                        SemanticTokensRegistrationOptions {
                            text_document_registration_options: {
                                TextDocumentRegistrationOptions {
                                    document_selector: Some(vec![DocumentFilter {
                                        language: Some("tect".to_string()),
                                        scheme: Some("file".to_string()),
                                        pattern: None,
                                    }]),
                                }
                            },
                            semantic_tokens_options: SemanticTokensOptions {
                                work_done_progress_options: Default::default(),
                                legend: SemanticTokensLegend {
                                    token_types: TOKEN_TYPES.clone(),
                                    token_modifiers: vec![],
                                },
                                range: Some(false),
                                full: Some(SemanticTokensFullOptions::Bool(true)),
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
        // In a real app, you'd fetch the file content from memory.
        // For now, we'll assume we can read it.
        let uri = params.text_document.uri;
        let content = std::fs::read_to_string(uri.to_file_path().unwrap()).unwrap();

        let mut tokens = Vec::new();
        let mut last_line = 0;
        let mut last_start = 0;

        if let Ok(pairs) = TectParser::parse(Rule::program, &content) {
            for pair in pairs.flatten() {
                let token_type = match pair.as_rule() {
                    Rule::data_def | Rule::error_def | Rule::func_def => Some(0), // Keyword
                    Rule::ident => Some(3), // Default to variable
                    // You can add more logic here to distinguish Type vs Func
                    _ => None,
                };

                if let Some(type_idx) = token_type {
                    let (line, col) = pair.line_col();
                    let line = (line - 1) as u32;
                    let col = (col - 1) as u32;
                    let len = pair.as_str().len() as u32;

                    // Compute Deltas (Required by LSP spec)
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
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend { client });
    Server::new(stdin, stdout, socket).serve(service).await;
}
