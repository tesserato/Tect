use anyhow::{Context, Result};
use clap::Parser as ClapParser;
use dashmap::DashMap;
use pest::Parser;
use pest_derive::Parser;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use walkdir::WalkDir;

#[derive(Parser)]
#[grammar = "tect.pest"]
pub struct TectParser;

mod tests;

#[derive(Debug, Clone, Serialize)]
pub struct Node {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub relation: String,
}

#[derive(Debug, Default, Serialize)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SymbolInfo {
    pub kind: String,
    pub detail: String,
    pub docs: Option<String>,
}

pub struct TectAnalyzer {
    pub symbols: HashMap<String, SymbolInfo>,
    pub func_returns: HashMap<String, String>,
    pub graph: Graph,
}

impl TectAnalyzer {
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
            func_returns: HashMap::new(),
            graph: Graph::default(),
        }
    }

    pub fn analyze(&mut self, content: &str) -> Result<()> {
        let pairs = TectParser::parse(Rule::program, content).context("Parsing failed")?;
        let top_level = pairs.into_iter().next().unwrap().into_inner();

        let nodes_before = top_level.clone();
        for pair in nodes_before {
            self.collect_defs(pair);
        }

        let usage_before = top_level;
        for pair in usage_before {
            self.collect_usage(pair);
        }

        Ok(())
    }

    fn collect_defs(&mut self, pair: pest::iterators::Pair<Rule>) {
        let rule = pair.as_rule();
        if matches!(rule, Rule::data_def | Rule::error_def | Rule::func_def) {
            let mut docs = Vec::new();
            let mut name = String::new();
            let mut ret_union = Vec::new();
            let mut input_type = String::new();

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
                    Rule::type_ident if name.is_empty() => name = inner.as_str().to_string(),
                    Rule::type_ident => input_type = inner.as_str().to_string(),
                    Rule::type_union => {
                        for type_pair in inner.into_inner() {
                            if type_pair.as_rule() == Rule::type_ident {
                                ret_union.push(type_pair.as_str().trim().to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }

            if !name.is_empty() {
                let kind = if rule == Rule::data_def {
                    "Data"
                } else if rule == Rule::error_def {
                    "Error"
                } else {
                    self.func_returns
                        .insert(name.clone(), ret_union.join(" | "));
                    "Function"
                };

                let doc_str = if docs.is_empty() {
                    None
                } else {
                    Some(docs.join("\n"))
                };

                self.symbols.insert(
                    name.clone(),
                    SymbolInfo {
                        kind: kind.into(),
                        detail: if ret_union.is_empty() {
                            name.clone()
                        } else {
                            ret_union.join(" | ")
                        },
                        docs: doc_str.clone(),
                    },
                );

                let id = format!("def:{}", name);
                self.graph.nodes.push(Node {
                    id: id.clone(),
                    kind: kind.into(),
                    label: name.clone(),
                    metadata: doc_str,
                });

                if rule == Rule::func_def {
                    if !input_type.is_empty() {
                        self.graph.edges.push(Edge {
                            source: format!("def:{}", input_type),
                            target: id.clone(),
                            relation: "input_type".into(),
                        });
                    }
                    for ret_type in ret_union {
                        self.graph.edges.push(Edge {
                            source: id.clone(),
                            target: format!("def:{}", ret_type),
                            relation: "output_type".into(),
                        });
                    }
                }
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
                        Rule::var_ident | Rule::type_ident => {
                            idents.push(inner.as_str().to_string())
                        }
                        _ => {}
                    }
                }
                if !idents.is_empty() {
                    let name = idents[0].clone();
                    let (kind, detail) = if rule == Rule::instantiation {
                        ("Variable", idents.get(1).cloned().unwrap_or_default())
                    } else if rule == Rule::assignment {
                        (
                            "Variable",
                            self.func_returns
                                .get(&idents[1])
                                .cloned()
                                .unwrap_or("Unknown".into()),
                        )
                    } else {
                        ("Side Effect", "Call".into())
                    };

                    let doc_str = if docs.is_empty() {
                        None
                    } else {
                        Some(docs.join("\n"))
                    };

                    self.symbols.insert(
                        name.clone(),
                        SymbolInfo {
                            kind: kind.into(),
                            detail: detail.clone(),
                            docs: doc_str.clone(),
                        },
                    );

                    let id = if rule == Rule::call {
                        format!("call:{}", name)
                    } else {
                        format!("var:{}", name)
                    };

                    self.graph.nodes.push(Node {
                        id: id.clone(),
                        kind: kind.into(),
                        label: name.clone(),
                        metadata: doc_str,
                    });

                    match rule {
                        Rule::instantiation => {
                            self.graph.edges.push(Edge {
                                source: format!("def:{}", detail),
                                target: id,
                                relation: "type_definition".into(),
                            });
                        }
                        Rule::assignment => {
                            let func_name = &idents[1];
                            let arg_name = &idents[2];
                            self.graph.edges.push(Edge {
                                source: format!("var:{}", arg_name),
                                target: format!("def:{}", func_name),
                                relation: "argument_flow".into(),
                            });
                            self.graph.edges.push(Edge {
                                source: format!("def:{}", func_name),
                                target: id,
                                relation: "result_flow".into(),
                            });
                        }
                        Rule::call => {
                            let arg_name = &idents[1];
                            self.graph.edges.push(Edge {
                                source: format!("var:{}", arg_name),
                                target: format!("def:{}", name),
                                relation: "argument_flow".into(),
                            });
                        }
                        _ => {}
                    }
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
    #[allow(dead_code)]
    client: Client,
    document_map: DashMap<Url, String>,
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
    async fn hover(&self, p: HoverParams) -> LspResult<Option<Hover>> {
        let uri = p.text_document_position_params.text_document.uri;
        let pos = p.text_document_position_params.position;
        let Some(content) = self.document_map.get(&uri) else {
            return Ok(None);
        };
        let mut a = TectAnalyzer::new();
        if a.analyze(&content).is_err() {
            return Ok(None);
        }
        if let Ok(pairs) = TectParser::parse(Rule::program, &content) {
            for pair in pairs.flatten() {
                if !matches!(
                    pair.as_rule(),
                    Rule::type_ident
                        | Rule::var_ident
                        | Rule::kw_data
                        | Rule::kw_error
                        | Rule::kw_func
                        | Rule::kw_match
                        | Rule::kw_for
                        | Rule::wildcard
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
                            "### {}: `{}`\n**Type/Detail**: `{}`{}",
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
                            Rule::kw_data => "### Keyword: `data`".into(),
                            Rule::kw_error => "### Keyword: `error`".into(),
                            Rule::kw_func => "### Keyword: `function`".into(),
                            Rule::kw_match => "### Keyword: `match`".into(),
                            Rule::kw_for => "### Keyword: `for`".into(),
                            Rule::wildcard => "### Pattern: `Wildcard` (Exhaustive match)".into(),
                            _ => format!("### Symbol: `{}`", word),
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
    ) -> LspResult<Option<SemanticTokensResult>> {
        let uri = p.text_document.uri;
        let Some(content) = self.document_map.get(&uri) else {
            return Ok(None);
        };
        let mut a = TectAnalyzer::new();
        if a.analyze(&content).is_err() {
            return Ok(None);
        }
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
                    | Rule::kw_break => Some(0),
                    Rule::type_ident => Some(
                        match a.symbols.get(pair.as_str()).map(|s| s.kind.as_str()) {
                            Some("Data") => 1,
                            Some("Function") => 2,
                            Some("Error") => 4,
                            _ => 1,
                        },
                    ),
                    Rule::var_ident => Some(3),
                    Rule::number | Rule::wildcard => Some(4),
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

#[derive(ClapParser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Optional path for CLI analysis. If omitted, starts LSP.
    input: Option<PathBuf>,

    #[arg(short, long)]
    output: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // If the binary is started without arguments, Args::try_parse will fail
    // and we should default to LSP mode.
    let args_res = Args::try_parse();

    if let Ok(args) = args_res {
        if let Some(input_path) = args.input {
            // CLI Mode
            let mut analyzer = TectAnalyzer::new();
            let files = if input_path.is_dir() {
                WalkDir::new(input_path)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().map_or(false, |ext| ext == "tect"))
                    .map(|e| e.path().to_path_buf())
                    .collect::<Vec<_>>()
            } else {
                vec![input_path]
            };

            for file in files {
                let content = fs::read_to_string(&file)?;
                analyzer.analyze(&content)?;
            }

            let json_output = serde_json::to_string_pretty(&analyzer.graph)?;
            if let Some(out_path) = args.output {
                fs::write(out_path, json_output)?;
            } else {
                println!("{}", json_output);
            }
            return Ok(());
        }
    }

    // Default: LSP Mode
    let (service, socket) = LspService::new(|client| Backend {
        client,
        document_map: DashMap::new(),
    });
    Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
        .serve(service)
        .await;

    Ok(())
}
