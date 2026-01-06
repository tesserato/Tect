//! # Tect Semantic Analyzer
//!
//! Responsible for transforming raw Tect source code into a [ProgramStructure].
//! Designed for fault tolerance to support LSP background processing.

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
    pub symbol_table: HashMap<u32, SymbolMetadata>,
}

impl TectAnalyzer {
    pub fn new() -> Self {
        Self {
            symbol_table: HashMap::new(),
        }
    }

    pub fn analyze(&mut self, content: &str) -> Result<ProgramStructure> {
        let mut structure = ProgramStructure::default();
        let pairs = TectParser::parse(Rule::program, content)
            .context("Syntax Error")?
            .next()
            .unwrap();
        let statements: Vec<Pair<Rule>> = pairs.into_inner().collect();

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

        for pair in &statements {
            match pair.as_rule() {
                Rule::func_def => self.link_function_contracts(pair, &mut structure)?,
                Rule::flow_step => structure.flow.push(pair.as_str().trim().to_string()),
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

    fn define_type(
        &mut self,
        pair: &Pair<Rule>,
        kw: &str,
        structure: &mut ProgramStructure,
    ) -> Result<()> {
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
        structure.artifacts.insert(name, kind);
        self.symbol_table.insert(
            uid,
            SymbolMetadata {
                definition_span: Self::map_span(&name_token),
                occurrences: Vec::new(),
            },
        );
        Ok(())
    }

    fn define_group(&mut self, pair: &Pair<Rule>, structure: &mut ProgramStructure) -> Result<()> {
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
        structure.groups.insert(name, group);
        Ok(())
    }

    fn define_function_skeleton(
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
        structure.catalog.insert(name, function);
        Ok(())
    }

    fn link_function_contracts(
        &mut self,
        _pair: &Pair<Rule>,
        _structure: &mut ProgramStructure,
    ) -> Result<()> {
        Ok(())
    }
}
