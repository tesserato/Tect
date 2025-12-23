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
    pub group: String,
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
    current_group: String,
}

impl TectAnalyzer {
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
            func_returns: HashMap::new(),
            graph: Graph::default(),
            current_group: "global".to_string(),
        }
    }

    pub fn analyze(&mut self, content: &str) -> Result<()> {
        let pairs = TectParser::parse(Rule::program, content).context("Parsing failed")?;
        let top_level = pairs.into_iter().next().unwrap().into_inner();

        for pair in top_level {
            self.process_pair(pair);
        }

        Ok(())
    }

    fn process_pair(&mut self, pair: pest::iterators::Pair<Rule>) {
        match pair.as_rule() {
            Rule::group_block => {
                let mut inner = pair.into_inner();
                let _kw = inner.next(); // group keyword
                let name_pair = inner.next().unwrap();
                let group_name = name_pair.as_str().to_string();

                // Register group as a symbol so we can hover it
                self.symbols.insert(
                    group_name.clone(),
                    SymbolInfo {
                        kind: "Group".into(),
                        detail: format!("Architectural Module: {}", group_name),
                        docs: Some(format!(
                            "Logical container for related architectural components."
                        )),
                    },
                );

                let old_group = self.current_group.clone();
                self.current_group = group_name;
                for p in inner {
                    self.process_pair(p);
                }
                self.current_group = old_group;
            }
            Rule::data_def | Rule::error_def | Rule::func_def => self.collect_defs(pair),
            Rule::instantiation | Rule::assignment | Rule::call | Rule::break_stmt => {
                self.collect_usage(pair)
            }
            Rule::for_stmt | Rule::match_stmt => {
                for inner in pair.into_inner() {
                    self.process_pair(inner);
                }
            }
            Rule::match_arm => {
                for inner in pair.into_inner().skip(1) {
                    self.process_pair(inner);
                }
            }
            _ => {}
        }
    }

    fn collect_defs(&mut self, pair: pest::iterators::Pair<Rule>) {
        let rule = pair.as_rule();
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
                    for tp in inner.into_inner() {
                        if tp.as_rule() == Rule::type_ident {
                            ret_union.push(tp.as_str().trim().to_string());
                        }
                    }
                }
                _ => {}
            }
        }

        if !name.is_empty() {
            let kind = match rule {
                Rule::data_def => "Data",
                Rule::error_def => "Error",
                _ => {
                    self.func_returns
                        .insert(name.clone(), ret_union.join(" | "));
                    "Function"
                }
            };
            let doc_str = if docs.is_empty() {
                None
            } else {
                Some(docs.join("\n\n"))
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

            self.graph.nodes.push(Node {
                id: format!("def:{}", name),
                kind: kind.into(),
                label: name.clone(),
                metadata: doc_str,
                group: self.current_group.clone(),
            });

            if rule == Rule::func_def {
                let id = format!("def:{}", name);
                if !input_type.is_empty() {
                    self.graph.edges.push(Edge {
                        source: format!("def:{}", input_type),
                        target: id.clone(),
                        relation: "input_type".into(),
                    });
                }
                for ret in ret_union {
                    self.graph.edges.push(Edge {
                        source: id.clone(),
                        target: format!("def:{}", ret),
                        relation: "output_type".into(),
                    });
                }
            }
        }
    }

    fn collect_usage(&mut self, pair: pest::iterators::Pair<Rule>) {
        let rule = pair.as_rule();
        let mut idents = Vec::new();
        let mut inline_group = None;
        let mut docs = Vec::new();

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
                Rule::var_ident | Rule::type_ident => idents.push(inner.as_str().to_string()),
                Rule::group_tag => {
                    inline_group = Some(inner.as_str().trim_start_matches('@').to_string())
                }
                _ => {}
            }
        }

        if !idents.is_empty() || rule == Rule::break_stmt {
            let name = idents
                .get(0)
                .cloned()
                .unwrap_or_else(|| "break".to_string());
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
            } else if rule == Rule::break_stmt {
                ("Logic", "Exit Loop".into())
            } else {
                ("Side Effect", "Call".into())
            };

            let group = inline_group.unwrap_or_else(|| self.current_group.clone());
            let id = if rule == Rule::call {
                format!("call:{}", name)
            } else {
                format!("var:{}", name)
            };
            let doc_str = if docs.is_empty() {
                None
            } else {
                Some(docs.join("\n\n"))
            };

            self.graph.nodes.push(Node {
                id: id.clone(),
                kind: kind.into(),
                label: name.clone(),
                metadata: doc_str.clone(),
                group,
            });
            self.symbols.insert(
                name,
                SymbolInfo {
                    kind: kind.into(),
                    detail: detail.clone(),
                    docs: doc_str,
                },
            );

            match rule {
                Rule::instantiation => {
                    self.graph.edges.push(Edge {
                        source: format!("def:{}", detail),
                        target: id,
                        relation: "type_definition".into(),
                    });
                }
                Rule::assignment => {
                    self.graph.edges.push(Edge {
                        source: format!("var:{}", idents[2]),
                        target: format!("def:{}", idents[1]),
                        relation: "argument_flow".into(),
                    });
                    self.graph.edges.push(Edge {
                        source: format!("def:{}", idents[1]),
                        target: id,
                        relation: "result_flow".into(),
                    });
                }
                Rule::call => {
                    self.graph.edges.push(Edge {
                        source: format!("var:{}", idents[1]),
                        target: format!("def:{}", idents[0]),
                        relation: "argument_flow".into(),
                    });
                }
                _ => {}
            }
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
                                        SemanticTokenType::KEYWORD,   // 0
                                        SemanticTokenType::TYPE,      // 1
                                        SemanticTokenType::FUNCTION,  // 2
                                        SemanticTokenType::VARIABLE,  // 3
                                        SemanticTokenType::ENUM,      // 4
                                        SemanticTokenType::DECORATOR, // 5
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
        let _ = a.analyze(&content);
        if let Ok(pairs) = TectParser::parse(Rule::program, &content) {
            for pair in pairs.flatten() {
                let rule = pair.as_rule();
                if !matches!(
                    rule,
                    Rule::type_ident
                        | Rule::var_ident
                        | Rule::kw_data
                        | Rule::kw_error
                        | Rule::kw_func
                        | Rule::kw_match
                        | Rule::kw_for
                        | Rule::kw_break
                        | Rule::kw_in
                        | Rule::kw_group
                        | Rule::wildcard
                        | Rule::group_tag
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
                    let lookup = word.trim_start_matches('@');
                    let val = if let Some(info) = a.symbols.get(lookup) {
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
                        match rule {
                            Rule::kw_data => "### Keyword: `data`\nDefines a domain entity.".into(),
                            Rule::kw_error => "### Keyword: `error`\nDefines a failure state.".into(),
                            Rule::kw_func => "### Keyword: `function`\nDefines a transformation contract.".into(),
                            Rule::kw_match => "### Keyword: `match`\nArchitectural branching.".into(),
                            Rule::kw_for => "### Keyword: `for`\nRepetition loop.".into(),
                            Rule::kw_group => "### Keyword: `group`\nStarts an architectural cluster block.".into(),
                            Rule::group_tag => format!("### Group Attachment: `@` \nAssigns this specific node to the group: `{}`", lookup).into(),
                            Rule::wildcard => "### Wildcard: `_`\nCatch-all match pattern.".into(),
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
                    Rule::type_ident => Some(
                        match a.symbols.get(pair.as_str()).map(|s| s.kind.as_str()) {
                            Some("Data") => 1,
                            Some("Function") => 2,
                            Some("Error") => 4,
                            _ => 1,
                        },
                    ),
                    Rule::var_ident => Some(
                        match a.symbols.get(pair.as_str()).map(|s| s.kind.as_str()) {
                            Some("Group") => 4,
                            _ => 3,
                        },
                    ),
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

#[derive(ClapParser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    input: Option<PathBuf>,
    #[arg(short, long)]
    output: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args_res = Args::try_parse();
    if let Ok(args) = args_res {
        if let Some(input_path) = args.input {
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
    let (service, socket) = LspService::new(|client| Backend {
        client,
        document_map: DashMap::new(),
    });
    Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
        .serve(service)
        .await;
    Ok(())
}
