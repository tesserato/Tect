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

mod tests;

#[derive(Debug, Clone, PartialEq)]
pub struct SymbolInfo {
    pub kind: String,
    pub detail: String,
    pub docs: Option<String>,
}

pub struct TectAnalyzer {
    pub symbols: HashMap<String, SymbolInfo>,
    pub func_returns: HashMap<String, String>,
}

impl TectAnalyzer {
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
            func_returns: HashMap::new(),
        }
    }

    pub fn analyze(&mut self, content: &str) {
        if let Ok(pairs) = TectParser::parse(Rule::program, content) {
            let top_level = pairs.into_iter().next().unwrap().into_inner();

            for pair in top_level.clone() {
                self.collect_defs(pair);
            }
            for pair in top_level {
                self.collect_usage(pair);
            }
        }
    }

    fn collect_defs(&mut self, pair: pest::iterators::Pair<Rule>) {
        let rule = pair.as_rule();
        if matches!(rule, Rule::data_def | Rule::error_def | Rule::func_def) {
            let mut docs = Vec::new();
            let mut name = String::new();
            let mut ret = String::new();
            for inner in pair.into_inner() {
                match inner.as_rule() {
                    Rule::doc_line => docs.push(
                        inner
                            .into_inner()
                            .next()
                            .unwrap()
                            .as_str()
                            .trim_start_matches('#')
                            .trim()
                            .to_string(),
                    ),
                    Rule::ident if name.is_empty() => name = inner.as_str().to_string(),
                    Rule::type_union => ret = inner.as_str().to_string(),
                    _ => {}
                }
            }
            if !name.is_empty() {
                let kind = if rule == Rule::data_def {
                    "Data"
                } else if rule == Rule::error_def {
                    "Error"
                } else {
                    self.func_returns.insert(name.clone(), ret.clone());
                    "Function"
                };
                self.symbols.insert(
                    name.clone(),
                    SymbolInfo {
                        kind: kind.into(),
                        detail: if ret.is_empty() { name } else { ret },
                        docs: if docs.is_empty() {
                            None
                        } else {
                            Some(docs.join("\n\n"))
                        },
                    },
                );
            }
        }
    }

    fn collect_usage(&mut self, pair: pest::iterators::Pair<Rule>) {
        match pair.as_rule() {
            Rule::instantiation | Rule::assignment | Rule::call => {
                let mut docs = Vec::new();
                let mut idents = Vec::new();
                let rule = pair.as_rule();
                for inner in pair.into_inner() {
                    match inner.as_rule() {
                        Rule::doc_line => docs.push(
                            inner
                                .into_inner()
                                .next()
                                .unwrap()
                                .as_str()
                                .trim_start_matches('#')
                                .trim()
                                .to_string(),
                        ),
                        Rule::ident => idents.push(inner.as_str().to_string()),
                        _ => {}
                    }
                }
                if !idents.is_empty() {
                    let name = idents[0].clone();
                    let detail = if rule == Rule::instantiation {
                        idents.get(1).cloned().unwrap_or_default()
                    } else if rule == Rule::assignment {
                        self.func_returns
                            .get(&idents[1])
                            .cloned()
                            .unwrap_or("Unknown".into())
                    } else {
                        "Side Effect".into()
                    };
                    self.symbols.insert(
                        name,
                        SymbolInfo {
                            kind: "Variable".into(),
                            detail,
                            docs: if docs.is_empty() {
                                None
                            } else {
                                Some(docs.join("\n\n"))
                            },
                        },
                    );
                }
            }
            Rule::for_stmt | Rule::match_stmt | Rule::match_arm => {
                for inner in pair.into_inner() {
                    self.collect_usage(inner);
                }
            }
            _ => {}
        }
    }
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
                                        SemanticTokenType::KEYWORD,
                                        SemanticTokenType::TYPE,
                                        SemanticTokenType::FUNCTION,
                                        SemanticTokenType::VARIABLE,
                                        SemanticTokenType::ENUM,
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
    async fn hover(&self, p: HoverParams) -> Result<Option<Hover>> {
        let uri = p.text_document_position_params.text_document.uri;
        let pos = p.text_document_position_params.position;
        let Some(content) = self.document_map.get(&uri) else {
            return Ok(None);
        };
        let mut a = TectAnalyzer::new();
        a.analyze(&content);
        if let Ok(pairs) = TectParser::parse(Rule::program, &content) {
            for pair in pairs.flatten() {
                if !matches!(
                    pair.as_rule(),
                    Rule::ident
                        | Rule::kw_data
                        | Rule::kw_error
                        | Rule::kw_func
                        | Rule::kw_match
                        | Rule::kw_for
                        | Rule::kw_is
                ) {
                    continue;
                }
                let (l, c) = pair.line_col();
                let (line, col) = (l as u32 - 1, c as u32 - 1);
                if pos.line == line
                    && pos.character >= col
                    && pos.character < (col + pair.as_str().len() as u32)
                {
                    let word = pair.as_str();
                    let val = if let Some(info) = a.symbols.get(word) {
                        format!(
                            "### {}: `{}`\n**Type**: `{}`{}",
                            info.kind,
                            word,
                            info.detail,
                            info.docs
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
                            _ => format!("### Identifier: `{}`", word),
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
        p: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = p.text_document.uri;
        let Some(content) = self.document_map.get(&uri) else {
            return Ok(None);
        };
        let mut a = TectAnalyzer::new();
        a.analyze(&content);
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
                    | Rule::kw_is
                    | Rule::kw_break => Some(0),
                    Rule::ident => Some(
                        match a.symbols.get(pair.as_str()).map(|s| s.kind.as_str()) {
                            Some("Data") => 1,
                            Some("Function") => 2,
                            Some("Error") => 4,
                            _ => 3,
                        },
                    ),
                    Rule::number => Some(4),
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
