//! # Tect Semantic Analyzer
//!
//! Responsible for transforming raw Tect source code into a [ProgramStructure].
//! Performs three passes: symbol discovery, linking, and cleanup.

use crate::models::*;
use pest::iterators::{Pair, Pairs};
use pest::Parser;
use pest_derive::Parser;
use std::sync::Arc;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, DiagnosticTag, Position, Range};

#[derive(Parser)]
#[grammar = "tect.pest"]
pub struct TectParser;

pub struct TectAnalyzer;

impl TectAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Analyzes source code. Returns the structure even on semantic errors,
    /// collecting diagnostics along the way.
    pub fn analyze(&mut self, content: &str) -> ProgramStructure {
        let mut structure = ProgramStructure::default();

        let parse_res = TectParser::parse(Rule::program, content);

        let pairs = match parse_res {
            Ok(mut p) => p.next().unwrap(),
            Err(e) => {
                structure.diagnostics.push(self.pest_error_to_diagnostic(e));
                return structure;
            }
        };

        let statements: Vec<Pair<Rule>> = pairs.into_inner().collect();

        // Pass 1: Global Discovery (Definitions)
        for pair in &statements {
            match pair.as_rule() {
                Rule::const_def => self.define_type(pair, "constant", &mut structure, content),
                Rule::var_def => self.define_type(pair, "variable", &mut structure, content),
                Rule::err_def => self.define_type(pair, "error", &mut structure, content),
                Rule::group_def => self.define_group(pair, &mut structure, content),
                Rule::func_def => self.define_function_skeleton(pair, &mut structure, content),
                _ => {}
            }
        }

        // Pass 2: Linking Contracts, Occurrences, and Flow
        for pair in &statements {
            match pair.as_rule() {
                Rule::func_def => self.link_function_contracts(pair, &mut structure, content),
                Rule::flow_step => {
                    let name = pair.as_str().trim();
                    if !name.is_empty() {
                        let span = Self::map_span(pair);
                        structure.flow.push(FlowStep {
                            function_name: name.to_string(),
                            span,
                        });

                        // Register occurrence of the function name
                        if let Some(func) = structure.catalog.get(name) {
                            self.add_occurrence(func.uid, span, &mut structure);
                        } else {
                            structure.diagnostics.push(self.semantic_error(
                                span,
                                format!("Undefined function: '{}'", name),
                                content,
                            ));
                        }
                    }
                }
                _ => {}
            }
        }

        // Pass 3: Validation (Unused Symbols)
        for meta in structure.symbol_table.values() {
            if meta.occurrences.len() == 1 {
                let range = self.calculate_range(meta.definition_span, content);
                structure.diagnostics.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::WARNING),
                    code: Some(tower_lsp::lsp_types::NumberOrString::String(
                        "unused".to_string(),
                    )),
                    source: Some("tect".to_string()),
                    message: format!("Unused symbol: '{}'", meta.name),
                    tags: Some(vec![DiagnosticTag::UNNECESSARY]),
                    ..Default::default()
                });
            }
        }

        structure
    }

    fn map_span(p: &Pair<Rule>) -> Span {
        let s = p.as_span();
        Span {
            start: s.start(),
            end: s.end(),
        }
    }

    fn add_occurrence(&self, uid: u32, span: Span, structure: &mut ProgramStructure) {
        if let Some(meta) = structure.symbol_table.get_mut(&uid) {
            meta.occurrences.push(span);
        }
    }

    fn check_duplicate(
        &self,
        name: &str,
        span: Span,
        structure: &mut ProgramStructure,
        content: &str,
    ) -> bool {
        if structure.artifacts.contains_key(name)
            || structure.groups.contains_key(name)
            || structure.catalog.contains_key(name)
        {
            structure.diagnostics.push(Diagnostic {
                range: self.calculate_range(span, content),
                severity: Some(DiagnosticSeverity::ERROR),
                message: format!("Symbol '{}' is already defined.", name),
                ..Default::default()
            });
            return true;
        }
        false
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
        &self,
        pair: &Pair<Rule>,
        kw: &str,
        structure: &mut ProgramStructure,
        content: &str,
    ) {
        let mut inner = pair.clone().into_inner();
        let doc_str = Self::collect_docs(&mut inner);
        let _kw = inner.next().unwrap();
        let name_p = inner.next().unwrap();
        let name = name_p.as_str().to_string();
        let span = Self::map_span(&name_p);

        if self.check_duplicate(&name, span, structure, content) {
            return;
        }

        let kind = match kw {
            "constant" => Kind::Constant(Arc::new(Constant::new(name.clone(), doc_str))),
            "variable" => Kind::Variable(Arc::new(Variable::new(name.clone(), doc_str))),
            _ => Kind::Error(Arc::new(Error::new(name.clone(), doc_str))),
        };

        structure.symbol_table.insert(
            kind.uid(),
            SymbolMetadata {
                name: name.clone(),
                definition_span: span,
                occurrences: vec![span],
            },
        );
        structure.artifacts.insert(name, kind);
    }

    fn define_group(&self, pair: &Pair<Rule>, structure: &mut ProgramStructure, content: &str) {
        let mut inner = pair.clone().into_inner();
        let doc_str = Self::collect_docs(&mut inner);
        let _kw = inner.next().unwrap();
        let name_p = inner.next().unwrap();
        let name = name_p.as_str().to_string();
        let span = Self::map_span(&name_p);

        if self.check_duplicate(&name, span, structure, content) {
            return;
        }

        let group = Arc::new(Group::new(name.clone(), doc_str));
        structure.symbol_table.insert(
            group.uid,
            SymbolMetadata {
                name: name.clone(),
                definition_span: span,
                occurrences: vec![span],
            },
        );
        structure.groups.insert(name, group);
    }

    fn define_function_skeleton(
        &self,
        pair: &Pair<Rule>,
        structure: &mut ProgramStructure,
        content: &str,
    ) {
        let mut inner = pair.clone().into_inner();
        let doc_str = Self::collect_docs(&mut inner);

        let mut group = None;
        if let Some(p) = inner.peek() {
            if p.as_rule() == Rule::ident {
                let g_name_p = inner.next().unwrap();
                let g_name = g_name_p.as_str();

                if let Some(g) = structure.groups.get(g_name).cloned() {
                    group = Some(g.clone());
                    self.add_occurrence(g.uid, Self::map_span(&g_name_p), structure);
                } else {
                    structure.diagnostics.push(self.semantic_error(
                        Self::map_span(&g_name_p),
                        format!("Undefined group: '{}'", g_name),
                        content,
                    ));
                }
            }
        }

        let _kw = inner.next().unwrap();
        let name_p = inner.next().unwrap();
        let name = name_p.as_str().to_string();
        let span = Self::map_span(&name_p);

        if self.check_duplicate(&name, span, structure, content) {
            return;
        }

        let function = Arc::new(Function::new_skeleton(name.clone(), doc_str, group));
        structure.symbol_table.insert(
            function.uid,
            SymbolMetadata {
                name: name.clone(),
                definition_span: span,
                occurrences: vec![span],
            },
        );
        structure.catalog.insert(name, function);
    }

    fn link_function_contracts(
        &self,
        pair: &Pair<Rule>,
        structure: &mut ProgramStructure,
        content: &str,
    ) {
        let mut inner = pair.clone().into_inner();
        // Skip docs and group (already handled in Pass 1)
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
                consumes = self.resolve_tokens(inner.next().unwrap(), structure, content);
            }
        }

        let mut produces = Vec::new();
        if let Some(outputs_pair) = inner.next() {
            for line in outputs_pair.into_inner() {
                let list = line.into_inner().next().unwrap();
                produces.push(self.resolve_tokens(list, structure, content));
            }
        }

        if let Some(func) = structure.catalog.get_mut(name) {
            let f = Arc::get_mut(func).unwrap();
            f.consumes = consumes;
            f.produces = produces;
        }
    }

    fn resolve_tokens(
        &self,
        pair: Pair<Rule>,
        structure: &mut ProgramStructure,
        content: &str,
    ) -> Vec<Token> {
        let mut tokens = Vec::new();
        for t_pair in pair.into_inner() {
            let inner = t_pair.into_inner().next().unwrap();
            let (name, card, span) = match inner.as_rule() {
                Rule::collection => {
                    let ident_p = inner.into_inner().next().unwrap();
                    (
                        ident_p.as_str(),
                        Cardinality::Collection,
                        Self::map_span(&ident_p),
                    )
                }
                _ => (inner.as_str(), Cardinality::Unitary, Self::map_span(&inner)),
            };

            let kind = if let Some(k) = structure.artifacts.get(name) {
                k.clone()
            } else {
                // Implicit Variable Creation
                structure.diagnostics.push(Diagnostic {
                    range: self.calculate_range(span, content),
                    severity: Some(DiagnosticSeverity::INFORMATION),
                    message: format!("Implicitly created variable '{}'.", name),
                    ..Default::default()
                });
                Kind::Variable(Arc::new(Variable::new(name.to_string(), None)))
            };

            // Register occurrence
            self.add_occurrence(kind.uid(), span, structure);
            tokens.push(Token::new(kind, card));
        }
        tokens
    }

    fn pest_error_to_diagnostic(&self, e: pest::error::Error<Rule>) -> Diagnostic {
        let (start, end) = match e.line_col {
            pest::error::LineColLocation::Pos((l, c)) => (
                Position::new(l as u32 - 1, c as u32 - 1),
                Position::new(l as u32 - 1, c as u32),
            ),
            pest::error::LineColLocation::Span((ls, cs), (le, ce)) => (
                Position::new(ls as u32 - 1, cs as u32 - 1),
                Position::new(le as u32 - 1, ce as u32 - 1),
            ),
        };

        Diagnostic {
            range: Range::new(start, end),
            severity: Some(DiagnosticSeverity::ERROR),
            message: format!("Syntax error: {}", e.variant.message()),
            ..Default::default()
        }
    }

    /// Helper to convert a byte-based Span into an LSP line/col Range.
    pub fn calculate_range(&self, span: Span, content: &str) -> Range {
        let mut line = 0;
        let mut col = 0;
        let mut byte = 0;
        let mut start_pos = Position::new(0, 0);
        let mut end_pos = Position::new(0, 0);

        for c in content.chars() {
            let char_len = c.len_utf8();
            if byte == span.start {
                start_pos = Position::new(line, col);
            }
            if byte == span.end {
                end_pos = Position::new(line, col);
                break;
            }
            if c == '\n' {
                line += 1;
                col = 0;
            } else {
                col += c.len_utf16() as u32;
            }
            byte += char_len;
        }
        if byte == span.end {
            end_pos = Position::new(line, col);
        }

        Range::new(start_pos, end_pos)
    }

    fn semantic_error(&self, span: Span, msg: String, content: &str) -> Diagnostic {
        Diagnostic {
            range: self.calculate_range(span, content),
            severity: Some(DiagnosticSeverity::ERROR),
            message: msg,
            ..Default::default()
        }
    }
}
