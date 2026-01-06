//! # Tect Semantic Analyzer
//!
//! Responsible for transforming raw Tect source code into a [ProgramStructure].

use crate::models::*;
use anyhow::{Context, Result};
use pest::iterators::Pair;
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

        // Pass 1: Global Definitions & Skeleton Creation
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

        // Pass 2: Linking & Flow Discovery
        for pair in &statements {
            match pair.as_rule() {
                Rule::func_def => self.link_function_contracts(pair, &mut structure)?,
                Rule::flow_step => {
                    let name = pair.as_str().trim();
                    structure.flow.push(name.to_string());
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

    /// Captures leading doc comments from a rule.
    fn collect_docs(inner: &mut pest::iterators::Pairs<Rule>) -> Option<String> {
        let mut docs = Vec::new();
        while let Some(p) = inner.peek() {
            if p.as_rule() == Rule::doc_line {
                let line = inner
                    .next()
                    .unwrap()
                    .as_str()
                    .trim_start_matches('#')
                    .trim_start_matches(' ');
                // Preserve trailing newlines by including the raw line break from the rule
                docs.push(line.trim_end().to_string());
            } else {
                break;
            }
        }
        if docs.is_empty() {
            None
        } else {
            Some(docs.join("\n"))
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

        let _kw_token = inner.next().unwrap();
        let name_token = inner.next().unwrap();
        let name = name_token.as_str().to_string();

        let kind = match kw {
            "constant" => Kind::Constant(Arc::new(Constant::new(name.clone(), doc_str))),
            "variable" => Kind::Variable(Arc::new(Variable::new(name.clone(), doc_str))),
            _ => Kind::Error(Arc::new(Error::new(name.clone(), doc_str))),
        };

        let uid = kind.uid();
        structure.symbol_table.insert(
            uid,
            SymbolMetadata {
                definition_span: Self::map_span(&name_token),
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
        let name_token = inner.next().unwrap();
        let name = name_token.as_str().to_string();

        let group = Arc::new(Group::new(name.clone(), doc_str));
        structure.symbol_table.insert(
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
        let name_token = inner.next().unwrap();
        let name = name_token.as_str().to_string();

        let function = Arc::new(Function::new_skeleton(name.clone(), doc_str, group));
        structure.symbol_table.insert(
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
