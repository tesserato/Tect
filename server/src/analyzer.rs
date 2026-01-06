//! # Tect Semantic Analyzer
//!
//! Responsible for transforming raw Tect source code into a [ProgramStructure].
//! Performs two passes: Discovery (Symbols) and Resolution (Linking).

use crate::models::*;
use anyhow::{Context, Result};
use pest::iterators::{Pair, Pairs};
use pest::Parser;
use pest_derive::Parser;
use std::sync::Arc;

#[derive(Parser)]
#[grammar = "tect.pest"]
pub struct TectParser;

pub struct TectAnalyzer;

impl TectAnalyzer {
    pub fn new() -> Self {
        Self
    }

    pub fn analyze(&mut self, content: &str) -> Result<ProgramStructure> {
        let mut structure = ProgramStructure::default();
        let pairs = TectParser::parse(Rule::program, content)
            .context("Syntax Error")?
            .next()
            .unwrap();
        let statements: Vec<Pair<Rule>> = pairs.into_inner().collect();

        // Pass 1: Global Discovery
        for pair in &statements {
            match pair.as_rule() {
                Rule::const_def => self.define_type(pair, "constant", &mut structure)?,
                Rule::var_def => self.define_type(pair, "variable", &mut structure)?,
                Rule::err_def => self.define_type(pair, "error", &mut structure)?,
                Rule::group_def => self.define_group(pair, &mut structure)?,
                Rule::func_def => self.define_function_skeleton(pair, &mut structure)?,
                _ => {}
            }
        }

        // Pass 2: Link Contracts & Resolve Flow
        for pair in &statements {
            match pair.as_rule() {
                Rule::func_def => self.link_function_contracts(pair, &mut structure)?,
                Rule::flow_step => {
                    let name = pair.as_str().trim();
                    if !name.is_empty() {
                        structure.flow.push(name.to_string());
                    }
                }
                _ => {}
            }
        }
        Ok(structure)
    }

    fn map_span(p: &Pair<Rule>) -> Span {
        let s = p.as_span();
        Span {
            start: s.start(),
            end: s.end(),
        }
    }

    fn collect_docs(inner: &mut Pairs<Rule>) -> Option<String> {
        let mut docs = Vec::new();
        while let Some(p) = inner.peek() {
            if p.as_rule() == Rule::doc_line {
                let raw = inner.next().unwrap().as_str();
                let content = raw
                    .trim_start_matches('#')
                    .trim_start_matches(' ')
                    .trim_end();
                docs.push(content.to_string());
            } else {
                break;
            }
        }
        if docs.is_empty() {
            None
        } else {
            Some(docs.join("  \n"))
        }
    }

    fn define_type(
        &mut self,
        pair: &Pair<Rule>,
        kw: &str,
        structure: &mut ProgramStructure,
    ) -> Result<()> {
        let mut inner = pair.clone().into_inner();
        let doc_str = Self::collect_docs(&mut inner);
        let _kw = inner.next().unwrap();
        let name_p = inner.next().unwrap();
        let name = name_p.as_str().to_string();

        let kind = match kw {
            "constant" => Kind::Constant(Arc::new(Constant::new(name.clone(), doc_str))),
            "variable" => Kind::Variable(Arc::new(Variable::new(name.clone(), doc_str))),
            _ => Kind::Error(Arc::new(Error::new(name.clone(), doc_str))),
        };

        structure.symbol_table.insert(
            kind.uid(),
            SymbolMetadata {
                definition_span: Self::map_span(&name_p),
                occurrences: Vec::new(),
            },
        );
        structure.artifacts.insert(name, kind);
        Ok(())
    }

    fn define_group(&mut self, pair: &Pair<Rule>, structure: &mut ProgramStructure) -> Result<()> {
        let mut inner = pair.clone().into_inner();
        let doc_str = Self::collect_docs(&mut inner);
        let _kw = inner.next().unwrap();
        let name_p = inner.next().unwrap();
        let name = name_p.as_str().to_string();

        let group = Arc::new(Group::new(name.clone(), doc_str));
        structure.symbol_table.insert(
            group.uid,
            SymbolMetadata {
                definition_span: Self::map_span(&name_p),
                occurrences: Vec::new(),
            },
        );
        structure.groups.insert(name, group);
        Ok(())
    }

    fn define_function_skeleton(
        &mut self,
        pair: &Pair<Rule>,
        structure: &mut ProgramStructure,
    ) -> Result<()> {
        let mut inner = pair.clone().into_inner();
        let doc_str = Self::collect_docs(&mut inner);

        let mut group = None;
        if let Some(p) = inner.peek() {
            if p.as_rule() == Rule::ident {
                group = structure
                    .groups
                    .get(inner.next().unwrap().as_str())
                    .cloned();
            }
        }

        let _kw = inner.next().unwrap();
        let name_p = inner.next().unwrap();
        let name = name_p.as_str().to_string();

        let function = Arc::new(Function::new_skeleton(name.clone(), doc_str, group));
        structure.symbol_table.insert(
            function.uid,
            SymbolMetadata {
                definition_span: Self::map_span(&name_p),
                occurrences: Vec::new(),
            },
        );
        structure.catalog.insert(name, function);
        Ok(())
    }

    fn link_function_contracts(
        &mut self,
        pair: &Pair<Rule>,
        structure: &mut ProgramStructure,
    ) -> Result<()> {
        let mut inner = pair.clone().into_inner();
        while let Some(p) = inner.peek() {
            if p.as_rule() == Rule::doc_line {
                inner.next();
            } else {
                break;
            }
        }
        if let Some(p) = inner.peek() {
            if p.as_rule() == Rule::ident {
                inner.next();
            }
        }
        let _kw = inner.next();
        let name = inner.next().unwrap().as_str();

        let mut consumes = Vec::new();
        if let Some(p) = inner.peek() {
            if p.as_rule() == Rule::token_list {
                consumes = self.resolve_tokens(inner.next().unwrap(), structure);
            }
        }

        let mut produces = Vec::new();
        if let Some(outputs_pair) = inner.next() {
            for line in outputs_pair.into_inner() {
                let list = line.into_inner().next().unwrap();
                produces.push(self.resolve_tokens(list, structure));
            }
        }

        if let Some(func) = structure.catalog.get_mut(name) {
            let f = Arc::get_mut(func).unwrap();
            f.consumes = consumes;
            f.produces = produces;
        }
        Ok(())
    }

    fn resolve_tokens(&self, pair: Pair<Rule>, structure: &ProgramStructure) -> Vec<Token> {
        let mut tokens = Vec::new();
        for t_pair in pair.into_inner() {
            let inner = t_pair.into_inner().next().unwrap();
            let (name, card) = match inner.as_rule() {
                Rule::collection => (
                    inner.into_inner().next().unwrap().as_str(),
                    Cardinality::Collection,
                ),
                _ => (inner.as_str(), Cardinality::Unitary),
            };

            let kind =
                structure.artifacts.get(name).cloned().unwrap_or_else(|| {
                    Kind::Variable(Arc::new(Variable::new(name.to_string(), None)))
                });
            tokens.push(Token::new(kind, card));
        }
        tokens
    }
}
