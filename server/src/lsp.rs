use crate::analyzer::{Rule, TectAnalyzer, TectParser};
use crate::models::Kind;
use dashmap::DashMap;
use pest::Parser;
use regex::Regex;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

/// The implementation of the Tect Language Server backend.
///
/// This handles the state of open documents and fulfills LSP requests
/// (hover, semantic tokens) by interacting with the `TectAnalyzer`.
pub struct Backend {
    #[allow(dead_code)]
    pub client: Client,
    /// Maps file URLs to their current in-memory content.
    pub document_map: DashMap<Url, String>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    /// Negotiates capabilities with the VS Code client upon connection.
    async fn initialize(&self, _: InitializeParams) -> LspResult<InitializeResult> {
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
                                        SemanticTokenType::KEYWORD,
                                        SemanticTokenType::TYPE,
                                        SemanticTokenType::FUNCTION,
                                        SemanticTokenType::VARIABLE,
                                        SemanticTokenType::ENUM,
                                        SemanticTokenType::DECORATOR,
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
        self.document_map
            .insert(p.text_document.uri, p.text_document.text);
    }

    async fn did_change(&self, p: DidChangeTextDocumentParams) {
        if let Some(c) = p.content_changes.into_iter().next() {
            self.document_map.insert(p.text_document.uri, c.text);
        }
    }

    /// Fulfils 'Hover' requests by providing architectural context for the token at the cursor.
    ///
    /// This implementation uses a fast Regex-based word detection to ensure tooltips appear
    /// even in syntactically invalid files (common during active typing).
    async fn hover(&self, p: HoverParams) -> LspResult<Option<Hover>> {
        let uri = p.text_document_position_params.text_document.uri;
        let pos = p.text_document_position_params.position;
        let Some(content) = self.document_map.get(&uri) else {
            return Ok(None);
        };

        let mut a = TectAnalyzer::new();
        let _ = a.analyze(&content);

        let lines: Vec<&str> = content.lines().collect();
        if let Some(line) = lines.get(pos.line as usize) {
            // Find words including the '@' prefix for groups
            let word_re = Regex::new(r"(@?[a-zA-Z0-9_:]+)").unwrap();
            for cap in word_re.find_iter(line) {
                if pos.character >= cap.start() as u32 && pos.character <= cap.end() as u32 {
                    let word = cap.as_str();
                    let lookup = word.trim_start_matches('@');

                    let val = if let Some(info) = a.symbols.get(lookup) {
                        let group_line = info
                            .group
                            .as_ref()
                            .map(|g| format!("\n**Group**: `{}`", g))
                            .unwrap_or_default();

                        format!(
                            "### {}: `{}`\n**Type**: `{}`{}{}",
                            info.kind,
                            lookup,
                            info.detail,
                            group_line,
                            info.docs
                                .as_ref()
                                .map(|d| format!("\n\n---\n\n{}", d))
                                .unwrap_or_default()
                        )
                    } else {
                        // Keyword tooltips for built-in Tect concepts
                        match lookup {
                            "data" => "### Keyword: `data`\nDefines a domain entity artifact.".into(),
                            "error" => "### Keyword: `error`\nDefines an architectural failure state.".into(),
                            "function" => "### Keyword: `function`\nDefines a transformation contract.".into(),
                            "match" => "### Keyword: `match`\nArchitectural branching based on result types.".into(),
                            "for" => "### Keyword: `for`\nRepresents a repetition loop.".into(),
                            "group" => "### Keyword: `group`\nLogical architectural container for modular organization.".into(),
                            "break" => "### Keyword: `break`\nExits the current repetition loop.".into(),
                            "None" => "### Built-in Type: `None`\nRepresents the absence of data (Architectural Unit).".into(),
                            "_" => "### Wildcard: `_`\nCatch-all match pattern for architectural branching.".into(),
                            _ if word.starts_with('@') => format!("### Group Assignment\nAssigns this statement to the module: `{}`", lookup),
                            _ => format!("### Symbol: `{}`", lookup),
                        }
                    };

                    return Ok(Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: val,
                        }),
                        range: Some(Range::new(
                            Position::new(pos.line, cap.start() as u32),
                            Position::new(pos.line, cap.end() as u32),
                        )),
                    }));
                }
            }
        }
        Ok(None)
    }

    /// Provides semantic token classification for sophisticated editor highlighting.
    /// Maps Tect architectural concepts (Functions, Groups, Data) to standard LSP token types.
    async fn semantic_tokens_full(
        &self,
        p: SemanticTokensParams,
    ) -> LspResult<Option<SemanticTokensResult>> {
        let uri = p.text_document.uri;
        let Some(content) = self.document_map.get(&uri) else {
            return Ok(None);
        };
        let mut a = TectAnalyzer::new();
        let _ = a.analyze(&content);
        let mut tokens = Vec::new();
        let (mut last_l, mut last_s) = (0, 0);

        if let Ok(pairs) = TectParser::parse(Rule::program, &content) {
            for pair in pairs.flatten() {
                let token_type = match pair.as_rule() {
                    Rule::kw_data
                    | Rule::kw_error
                    | Rule::kw_func
                    | Rule::kw_for
                    | Rule::kw_match
                    | Rule::kw_in
                    | Rule::kw_break
                    | Rule::kw_group => Some(0),
                    Rule::type_ident => Some(match a.symbols.get(pair.as_str()).map(|s| s.kind) {
                        Some(Kind::Data) => 1,
                        Some(Kind::Function) => 2,
                        Some(Kind::Error) => 4,
                        _ => 1,
                    }),
                    Rule::var_ident => Some(match a.symbols.get(pair.as_str()).map(|s| s.kind) {
                        Some(Kind::Group) => 1,
                        _ => 3,
                    }),
                    Rule::number | Rule::wildcard => Some(4),
                    Rule::group_tag => Some(5),
                    _ => None,
                };
                if let Some(idx) = token_type {
                    let (l, c) = pair.line_col();
                    let (line, col) = (l as u32 - 1, c as u32 - 1);
                    let delta_l = line - last_l;
                    let delta_s = if delta_l == 0 { col - last_s } else { col };
                    tokens.push(SemanticToken {
                        delta_line: delta_l,
                        delta_start: delta_s,
                        length: pair.as_str().len() as u32,
                        token_type: idx,
                        token_modifiers_bitset: 0,
                    });
                    last_l = line;
                    last_s = col;
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
