use crate::models::*;
use anyhow::{Context, Result};
use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Parser)]
#[grammar = "tect.pest"]
pub struct TectParser;

pub struct TectAnalyzer {
    pub registry_kinds: HashMap<String, Kind>,
    pub registry_groups: HashMap<String, Arc<Group>>,
    pub registry_functions: HashMap<String, Arc<Function>>,
    pub symbol_table: HashMap<u32, SymbolMetadata>,
    pub flow_nodes: Vec<Node>,
}

impl TectAnalyzer {
    pub fn new() -> Self {
        Self {
            registry_kinds: HashMap::new(),
            registry_groups: HashMap::new(),
            registry_functions: HashMap::new(),
            symbol_table: HashMap::new(),
            flow_nodes: Vec::new(),
        }
    }

    pub fn analyze(&mut self, content: &str) -> Result<()> {
        let program = TectParser::parse(Rule::program, content)
            .context("Syntax Error")?
            .next()
            .unwrap();

        let pairs: Vec<Pair<Rule>> = program.into_inner().collect();

        for pair in &pairs {
            match pair.as_rule() {
                Rule::const_def => self.define_type(pair, "constant")?,
                Rule::var_def => self.define_type(pair, "variable")?,
                Rule::err_def => self.define_type(pair, "error")?,
                Rule::group_def => self.define_group(pair)?,
                Rule::func_def => self.define_function_skeleton(pair)?,
                _ => {}
            }
        }

        for pair in &pairs {
            match pair.as_rule() {
                Rule::func_def => self.link_function_contracts(pair)?,
                Rule::flow_step => self.instantiate_node(pair)?,
                _ => {}
            }
        }

        Ok(())
    }

    fn map_span(p: &Pair<Rule>) -> Span {
        let s = p.as_span();
        Span {
            start: s.start(),
            end: s.end(),
        }
    }

    fn record_occurrence(&mut self, uid: u32, span: Span) {
        if let Some(meta) = self.symbol_table.get_mut(&uid) {
            meta.occurrences.push(span);
        }
    }

    fn define_type(&mut self, pair: &Pair<Rule>, kw: &str) -> Result<()> {
        let mut inner = pair.clone().into_inner();
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

        let _kw_token = inner.next().unwrap();
        let name_token = inner.next().unwrap();
        let name = name_token.as_str().to_string();
        let doc_str = if docs.is_empty() {
            None
        } else {
            Some(docs.join("\n"))
        };

        let kind = match kw {
            "constant" => Kind::Constant(Arc::new(Constant::new(name.clone(), doc_str))),
            "variable" => Kind::Variable(Arc::new(Variable::new(name.clone(), doc_str))),
            _ => Kind::Error(Arc::new(Error::new(name.clone(), doc_str))),
        };

        let uid = match &kind {
            Kind::Constant(c) => c.uid,
            Kind::Variable(v) => v.uid,
            Kind::Error(e) => e.uid,
        };

        self.registry_kinds.insert(name, kind);
        self.symbol_table.insert(
            uid,
            SymbolMetadata {
                definition_span: Self::map_span(&name_token),
                occurrences: Vec::new(),
            },
        );
        Ok(())
    }

    fn define_group(&mut self, pair: &Pair<Rule>) -> Result<()> {
        let mut inner = pair.clone().into_inner();
        let _kw = inner.next().unwrap();
        let name_token = inner.next().unwrap();
        let name = name_token.as_str().to_string();

        let group = Arc::new(Group::new(name.clone(), None));
        self.symbol_table.insert(
            group.uid,
            SymbolMetadata {
                definition_span: Self::map_span(&name_token),
                occurrences: Vec::new(),
            },
        );
        self.registry_groups.insert(name, group);
        Ok(())
    }

    fn define_function_skeleton(&mut self, pair: &Pair<Rule>) -> Result<()> {
        let mut inner = pair.clone().into_inner();
        while let Some(p) = inner.peek() {
            if p.as_rule() == Rule::doc_line {
                inner.next();
            } else {
                break;
            }
        }

        let mut group = None;
        if let Some(p) = inner.peek() {
            if p.as_rule() == Rule::ident {
                let g_token = inner.next().unwrap();
                group = self.registry_groups.get(g_token.as_str()).cloned();
                if let Some(g) = &group {
                    self.record_occurrence(g.uid, Self::map_span(&g_token));
                }
            }
        }

        let _kw = inner.next().unwrap();
        let name_token = inner.next().unwrap();
        let name = name_token.as_str().to_string();

        let function = Arc::new(Function::new_skeleton(name.clone(), group));
        self.symbol_table.insert(
            function.uid,
            SymbolMetadata {
                definition_span: Self::map_span(&name_token),
                occurrences: Vec::new(),
            },
        );
        self.registry_functions.insert(name, function);
        Ok(())
    }

    fn link_function_contracts(&mut self, _pair: &Pair<Rule>) -> Result<()> {
        Ok(())
    }

    fn instantiate_node(&mut self, pair: &Pair<Rule>) -> Result<()> {
        let name = pair.as_str().trim();
        if let Some(func) = self.registry_functions.get(name) {
            let node = Node::new(func.clone());
            self.record_occurrence(func.uid, Self::map_span(pair));
            self.symbol_table.insert(
                node.uid,
                SymbolMetadata {
                    definition_span: Self::map_span(pair),
                    occurrences: Vec::new(),
                },
            );
            self.flow_nodes.push(node);
        }
        Ok(())
    }
}
