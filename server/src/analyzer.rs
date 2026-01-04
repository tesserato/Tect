use crate::models::{Edge, Graph, Kind, Node, SymbolInfo};
use anyhow::{Context, Result};
use pest::Parser;
use pest_derive::Parser;
use regex::Regex;
use std::collections::HashMap;

#[derive(Parser)]
#[grammar = "tect.pest"]
pub struct TectParser;

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
        self.scrape_definitions(content);

        let pairs = TectParser::parse(Rule::program, content)
            .context("Formal parsing failed - check syntax rules")?;

        let top_level = pairs.into_iter().next().unwrap().into_inner();
        for pair in top_level {
            self.process_pair(pair);
        }
        Ok(())
    }

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

    fn scrape_definitions(&mut self, content: &str) {
        let re_const = Regex::new(r"(?m)((?:^\s*#.*\r?\n)*)\s*constant\s+([a-zA-Z0-9_]*)").unwrap();
        let re_var = Regex::new(r"(?m)((?:^\s*#.*\r?\n)*)\s*variable\s+([a-zA-Z0-9_]*)").unwrap();
        let re_err = Regex::new(r"(?m)((?:^\s*#.*\r?\n)*)\s*error\s+([a-zA-Z0-9_]*)").unwrap();
        let re_group = Regex::new(r"(?m)((?:^\s*#.*\r?\n)*)\s*group\s+([a-zA-Z0-9_]*)").unwrap();

        for cap in re_const.captures_iter(content) {
            self.symbols.insert(
                cap[2].to_string(),
                SymbolInfo {
                    kind: Kind::Data,
                    detail: "constant".to_string(),
                    docs: Self::parse_comments(&cap[1]),
                    group: None,
                },
            );
        }
        for cap in re_var.captures_iter(content) {
            self.symbols.insert(
                cap[2].to_string(),
                SymbolInfo {
                    kind: Kind::Variable,
                    detail: "variable".to_string(),
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
                    detail: "error".to_string(),
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
                    detail: format!("Group: {}", &cap[2]),
                    docs: Self::parse_comments(&cap[1]),
                    group: None,
                },
            );
        }
    }

    fn process_pair(&mut self, pair: pest::iterators::Pair<Rule>) {
        match pair.as_rule() {
            Rule::const_def | Rule::var_def | Rule::err_def => self.collect_type_def(pair),
            Rule::group_def => {
                let mut inner = pair.into_inner();
                let mut docs = Vec::new();
                while let Some(p) = inner.peek() {
                    if p.as_rule() == Rule::doc_line {
                        docs.push(
                            inner
                                .next()
                                .unwrap()
                                .as_str()
                                .trim_start_matches('#')
                                .trim()
                                .to_string(),
                        );
                    } else {
                        break;
                    }
                }
                let _kw = inner.next();
                let name = inner.next().unwrap().as_str();
                self.symbols.insert(
                    name.to_string(),
                    SymbolInfo {
                        kind: Kind::Group,
                        detail: "Architectural Group".into(),
                        docs: if docs.is_empty() {
                            None
                        } else {
                            Some(docs.join("\n"))
                        },
                        group: None,
                    },
                );
            }
            Rule::func_def => self.collect_func_def(pair),
            Rule::flow_step => {
                let name = pair.as_str().trim();
                self.graph.nodes.push(Node {
                    id: format!("flow:{}", name),
                    kind: Kind::Function,
                    label: name.to_string(),
                    metadata: None,
                    group: self.current_group.clone(),
                });
            }
            _ => {}
        }
    }

    fn collect_type_def(&mut self, pair: pest::iterators::Pair<Rule>) {
        let rule = pair.as_rule();
        let mut inner = pair.into_inner();
        let mut docs = Vec::new();

        while let Some(p) = inner.peek() {
            if p.as_rule() == Rule::doc_line {
                docs.push(
                    inner
                        .next()
                        .unwrap()
                        .as_str()
                        .trim_start_matches('#')
                        .trim()
                        .to_string(),
                );
            } else {
                break;
            }
        }

        let _kw = inner.next(); // kw_constant/variable/error
        let name = inner.next().unwrap().as_str();

        let kind = match rule {
            Rule::const_def => Kind::Data,
            Rule::var_def => Kind::Variable,
            Rule::err_def => Kind::Error,
            _ => Kind::Data,
        };

        let doc_str = if docs.is_empty() {
            None
        } else {
            Some(docs.join("\n"))
        };

        self.symbols.insert(
            name.to_string(),
            SymbolInfo {
                kind: kind.clone(),
                detail: format!("{:?}", kind),
                docs: doc_str.clone(),
                group: None,
            },
        );

        self.graph.nodes.push(Node {
            id: format!("def:{}", name),
            kind,
            label: name.to_string(),
            metadata: doc_str,
            group: "global".into(),
        });
    }

    fn collect_func_def(&mut self, pair: pest::iterators::Pair<Rule>) {
        let mut inner = pair.into_inner();
        let mut group = self.current_group.clone();

        // Skip docs
        while let Some(p) = inner.peek() {
            if p.as_rule() == Rule::doc_line {
                inner.next();
            } else {
                break;
            }
        }

        // Check for optional Group Prefix
        if let Some(p) = inner.peek() {
            if p.as_rule() == Rule::ident {
                group = inner.next().unwrap().as_str().to_string();
            }
        }

        let _kw = inner.next(); // kw_function
        let func_name = inner.next().unwrap().as_str();

        // Handle inputs (token_list)
        if let Some(p) = inner.peek() {
            if p.as_rule() == Rule::token_list {
                let list = inner.next().unwrap();
                for token_pair in list.into_inner() {
                    let token_inner = token_pair.into_inner().next().unwrap();
                    let type_name = if token_inner.as_rule() == Rule::collection {
                        token_inner.into_inner().next().unwrap().as_str()
                    } else {
                        token_inner.as_str()
                    };

                    self.graph.edges.push(Edge {
                        source: format!("def:{}", type_name),
                        target: format!("def:{}", func_name),
                        relation: "input".into(),
                    });
                }
            }
        }

        // Handle outputs (func_outputs)
        if let Some(outputs_pair) = inner.next() {
            for output_line in outputs_pair.into_inner() {
                let mut line_inner = output_line.into_inner();
                let _op = line_inner.next(); // > or |
                let list = line_inner.next().unwrap();
                for token_pair in list.into_inner() {
                    let token_inner = token_pair.into_inner().next().unwrap();
                    let type_name = if token_inner.as_rule() == Rule::collection {
                        token_inner.into_inner().next().unwrap().as_str()
                    } else {
                        token_inner.as_str()
                    };

                    self.graph.edges.push(Edge {
                        source: format!("def:{}", func_name),
                        target: format!("def:{}", type_name),
                        relation: "output".into(),
                    });
                }
            }
        }

        self.graph.nodes.push(Node {
            id: format!("def:{}", func_name),
            kind: Kind::Function,
            label: func_name.to_string(),
            metadata: None,
            group,
        });
    }
}
