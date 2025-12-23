use crate::models::{Edge, Graph, Kind, Node, SymbolInfo};
use anyhow::{Context, Result};
use pest::Parser;
use pest_derive::Parser;
use regex::Regex;
use std::collections::HashMap;

/// The primary parser driver utilizing the Pest grammar defined in `tect.pest`.
#[derive(Parser)]
#[grammar = "tect.pest"]
pub struct TectParser;

/// The core engine responsible for analyzing Tect source files.
///
/// It performs structural analysis to extract architectural entities and their
/// relationships, supporting both precise graph generation and fuzzy LSP features.
pub struct TectAnalyzer {
    /// Stores metadata for all identified symbols (Data, Functions, etc.).
    pub symbols: HashMap<String, SymbolInfo>,
    /// Tracks the return types of functions to enable flow inference.
    pub func_returns: HashMap<String, String>,
    /// The generated graph object suitable for JSON export.
    pub graph: Graph,
    /// Internal state tracking the current active architectural group.
    current_group: String,
}

impl TectAnalyzer {
    /// Initializes a new analyzer with a default "global" scope.
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
            func_returns: HashMap::new(),
            graph: Graph::default(),
            current_group: "global".to_string(),
        }
    }

    /// Orchestrates the analysis of Tect source content.
    ///
    /// This follows a multi-pass strategy:
    /// 1. Scrape definitions via Regex for immediate availability.
    /// 2. Formally parse the AST via Pest for relational integrity.
    pub fn analyze(&mut self, content: &str) -> Result<()> {
        self.scrape_definitions(content);

        let pairs = TectParser::parse(Rule::program, content)
            .context("Formal parsing failed - check syntax rules")?;

        let top_level = pairs.into_iter().next().unwrap().into_inner();
        for pair in top_level {
            self.process_pair(pair);
        }
        Ok(())
    }

    /// Internal helper to clean and format '#' comment blocks into Markdown.
    fn parse_comments(raw: &str) -> Option<String> {
        let docs: Vec<String> = raw
            .lines()
            .map(|l| l.trim().trim_start_matches('#').trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();

        if docs.is_empty() {
            None
        } else {
            Some(docs.join("\n\n"))
        }
    }

    /// Performs a high-speed scavenging pass using Regular Expressions.
    ///
    /// This allows the analyzer to populate the symbol table even when the file
    /// is syntactically incomplete, which is critical for LSP responsiveness.
    fn scrape_definitions(&mut self, content: &str) {
        let re_data = Regex::new(r"(?m)((?:^\s*#.*\r?\n)*)\s*data\s+([A-Z][a-zA-Z0-9_]*)").unwrap();
        let re_err = Regex::new(r"(?m)((?:^\s*#.*\r?\n)*)\s*error\s+([A-Z][a-zA-Z0-9_]*)").unwrap();
        let re_group =
            Regex::new(r"(?m)((?:^\s*#.*\r?\n)*)\s*group\s+([a-z][a-zA-Z0-9_]*)").unwrap();
        let re_func = Regex::new(r"(?m)((?:^\s*#.*\r?\n)*)\s*function\s+([A-Z][a-zA-Z0-9_]*)\s*\(([A-Z][a-zA-Z0-9_]*)\)\s*->\s*([^@\n\r{]+)").unwrap();

        for cap in re_data.captures_iter(content) {
            self.symbols.insert(
                cap[2].to_string(),
                SymbolInfo {
                    kind: Kind::Data,
                    detail: cap[2].to_string(),
                    docs: Self::parse_comments(&cap[1]),
                    group: None,
                },
            );
        }
        for cap in re_err.captures_iter(content) {
            self.symbols.insert(
                cap[2].to_string(),
                SymbolInfo {
                    kind: Kind::Error,
                    detail: cap[2].to_string(),
                    docs: Self::parse_comments(&cap[1]),
                    group: None,
                },
            );
        }
        for cap in re_group.captures_iter(content) {
            self.symbols.insert(
                cap[2].to_string(),
                SymbolInfo {
                    kind: Kind::Group,
                    detail: format!("Module: {}", &cap[2]),
                    docs: Self::parse_comments(&cap[1]),
                    group: None,
                },
            );
        }
        for cap in re_func.captures_iter(content) {
            let name = cap[2].to_string();
            let input = cap[3].to_string();
            let output = cap[4].trim().to_string();
            self.symbols.insert(
                name,
                SymbolInfo {
                    kind: Kind::Function,
                    detail: format!("{} -> {}", input, output),
                    docs: Self::parse_comments(&cap[1]),
                    group: None,
                },
            );
        }
    }

    /// Dispatches grammar rules to specific collectors during structural traversal.
    fn process_pair(&mut self, pair: pest::iterators::Pair<Rule>) {
        match pair.as_rule() {
            Rule::group_block => {
                let mut inner = pair.into_inner();
                let _kw = inner.next();
                if let Some(name_pair) = inner.next() {
                    let group_name = name_pair.as_str().to_string();
                    let old_group = self.current_group.clone();
                    self.current_group = group_name;
                    for p in inner {
                        self.process_pair(p);
                    }
                    self.current_group = old_group;
                }
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

    /// Analyzes formal definitions and maps their architectural signatures to nodes and edges.
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
            let detail = if rule == Rule::func_def {
                format!("{} -> {}", input_type, ret_union.join(" | "))
            } else {
                name.clone()
            };

            let kind = match rule {
                Rule::data_def => Kind::Data,
                Rule::error_def => Kind::Error,
                _ => {
                    self.func_returns
                        .insert(name.clone(), ret_union.join(" | "));
                    Kind::Function
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
                    kind,
                    detail,
                    docs: doc_str.clone(),
                    group: if self.current_group != "global" {
                        Some(self.current_group.clone())
                    } else {
                        None
                    },
                },
            );

            self.graph.nodes.push(Node {
                id: format!("def:{}", name),
                kind,
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
                    if ret != "None" {
                        self.graph.edges.push(Edge {
                            source: id.clone(),
                            target: format!("def:{}", ret),
                            relation: "output_type".into(),
                        });
                    }
                }
            }
        }
    }

    /// Maps runtime occurrences (variables, calls, logic) to graph instances.
    /// Performs type inference based on previous function return mappings.
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
                .first()
                .cloned()
                .unwrap_or_else(|| "break".to_string());

            let (kind, detail) = match rule {
                Rule::instantiation => (Kind::Variable, idents.get(1).cloned().unwrap_or_default()),
                Rule::assignment => {
                    let ret = self
                        .func_returns
                        .get(&idents[1])
                        .cloned()
                        .unwrap_or_else(|| "Unknown".into());
                    (Kind::Variable, ret)
                }
                Rule::break_stmt => (Kind::Logic, "Exit Loop".into()),
                Rule::call => (Kind::Function, "Procedural Call (None-Returning)".into()),
                _ => (Kind::Variable, "Unknown".into()),
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
                kind,
                label: name.clone(),
                metadata: doc_str.clone(),
                group: group.clone(),
            });
            self.symbols.insert(
                name,
                SymbolInfo {
                    kind,
                    detail: detail.clone(),
                    docs: doc_str,
                    group: if group != "global" { Some(group) } else { None },
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
                Rule::assignment if idents.len() >= 3 => {
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
                Rule::call if idents.len() >= 2 => {
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
