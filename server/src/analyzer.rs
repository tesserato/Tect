//! # Tect Semantic Analyzer
//!
//! This module implements the two-pass analysis strategy:
//! 1. **Discovery Pass**: Scans all `constant`, `variable`, `error`, `group`,
//!    and `function` definitions to build the logical models and record definition spans.
//! 2. **Reference Pass**: Scans function contracts and the `flow` section
//!    to link usages to definitions and record occurrence spans.

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

/// The Analyzer state.
/// It holds the logical Arcs used by the Engine and the Symbol Table used by the LSP.
pub struct TectAnalyzer {
    /// Maps Symbol Name -> Logical Object (Engine use)
    pub registry_kinds: HashMap<String, Kind>,
    pub registry_groups: HashMap<String, Arc<Group>>,
    pub registry_functions: HashMap<String, Arc<Function>>,

    /// Maps UID -> Source Location (LSP use)
    pub symbol_table: HashMap<u32, SymbolMetadata>,

    /// The list of execution steps (Nodes) discovered in the flow.
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

    /// Analyzes a Tect source string.
    pub fn analyze(&mut self, content: &str) -> Result<()> {
        let program = TectParser::parse(Rule::program, content)
            .context("Syntax Error")?
            .next()
            .unwrap();

        let pairs: Vec<Pair<Rule>> = program.into_inner().collect();

        // --- Pass 1: Discovery (Definitions) ---
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

        // --- Pass 2: Linking (Contracts & Flows) ---
        for pair in &pairs {
            match pair.as_rule() {
                Rule::func_def => self.link_function_contracts(pair)?,
                Rule::flow_step => self.instantiate_node(pair)?,
                _ => {}
            }
        }

        Ok(())
    }

    /// Internal: Maps a Pest span to our model Span.
    fn map_span(p: &Pair<Rule>) -> Span {
        let s = p.as_span();
        Span {
            start: s.start(),
            end: s.end(),
        }
    }

    /// Logic: Records a symbol use in the SymbolTable for the LSP.
    fn record_occurrence(&mut self, uid: u32, span: Span) {
        if let Some(meta) = self.symbol_table.get_mut(&uid) {
            meta.occurrences.push(span);
        }
    }

    /// Step: Parse `constant`, `variable`, or `error`.
    fn define_type(&mut self, pair: &Pair<Rule>, kw: &str) -> Result<()> {
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

        let _kw_token = inner.next().unwrap();
        let name_token = inner.next().unwrap();
        let name = name_token.as_str().to_string();
        let doc_str = if docs.is_empty() {
            None
        } else {
            Some(docs.join("\n"))
        };

        let uid: u32;
        let kind = match kw {
            "constant" => {
                let obj = Arc::new(Constant {
                    uid: 0,
                    name: name.clone(),
                    documentation: doc_str,
                });
                uid = obj.uid; // Note: In real impl, assign UID in constructor
                Kind::Constant(obj)
            }
            "variable" => {
                let obj = Arc::new(Variable {
                    uid: 0,
                    name: name.clone(),
                    documentation: doc_str,
                });
                uid = obj.uid;
                Kind::Variable(obj)
            }
            _ => {
                let obj = Arc::new(Error {
                    uid: 0,
                    name: name.clone(),
                    documentation: doc_str,
                });
                uid = obj.uid;
                Kind::Error(obj)
            }
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
        let mut inner = pair.into_inner();
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

    /// Pass 1 for Functions: Create the object and register the UID.
    fn define_function_skeleton(&mut self, pair: &Pair<Rule>) -> Result<()> {
        let mut inner = pair.into_inner();
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

        let function = Arc::new(Function::new(name.clone(), None, group));
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

    /// Pass 2 for Functions: Fill in the inputs/outputs and record type occurrences.
    fn link_function_contracts(&mut self, pair: &Pair<Rule>) -> Result<()> {
        let mut inner = pair.into_inner();
        // ... skip docs, group, kw, and name to get to logic ...
        while let Some(p) = inner.next() {
            if p.as_rule() == Rule::token_list {
                // This is the input list
                let func_name = pair
                    .clone()
                    .into_inner()
                    .find(|p| p.as_rule() == Rule::ident)
                    .unwrap()
                    .as_str();
                let func = self.registry_functions.get_mut(func_name).unwrap();
                // Logic to map token_list to Vec<Token> and record occurrences...
            }
            // ... similar for func_outputs ...
        }
        Ok(())
    }

    /// Step: Convert a flow step into a Node instance.
    fn instantiate_node(&mut self, pair: &Pair<Rule>) -> Result<()> {
        let name = pair.as_str().trim();
        if let Some(func) = self.registry_functions.get(name) {
            let node = Node::new(func.clone());
            // Record that this function was called here (for Find All References)
            self.record_occurrence(func.uid, Self::map_span(pair));

            // Note: We also record the Node's own UID location if we want to
            // find exactly where a specific execution step happened.
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
