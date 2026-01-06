//! # Tect Test Parser
//!
//! Integration test helper to verify Pest grammar consistency against model resolution.

use pest::Parser;
use pest_derive::Parser;
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;

use crate::models::{Cardinality, Constant, Error, Function, Group, Kind, Token, Variable};

#[derive(Parser)]
#[grammar = "tect.pest"]
pub struct TectParser;

/// Local registry to track defined symbols during the parsing pass.
struct SymbolRegistry {
    kinds: HashMap<String, Kind>,
    groups: HashMap<String, Arc<Group>>,
}

#[test]
fn main() {
    let unparsed_file = fs::read_to_string("../samples/dsbg.tect").expect("cannot read file");

    let program = TectParser::parse(Rule::program, &unparsed_file)
        .expect("unsuccessful parse")
        .next()
        .unwrap();

    let mut registry = SymbolRegistry {
        kinds: HashMap::new(),
        groups: HashMap::new(),
    };

    let mut functions: Vec<Arc<Function>> = Vec::new();

    // Pass: Symbol Definitions and Function Discovery
    for record in program.into_inner() {
        match record.as_rule() {
            Rule::const_def => {
                let (name, docs) = parse_def(record);
                registry.kinds.insert(
                    name.clone(),
                    Kind::Constant(Arc::new(Constant::new(name, docs))),
                );
            }
            Rule::var_def => {
                let (name, docs) = parse_def(record);
                registry.kinds.insert(
                    name.clone(),
                    Kind::Variable(Arc::new(Variable::new(name, docs))),
                );
            }
            Rule::err_def => {
                let (name, docs) = parse_def(record);
                registry
                    .kinds
                    .insert(name.clone(), Kind::Error(Arc::new(Error::new(name, docs))));
            }
            Rule::group_def => {
                let (name, docs) = parse_def(record);
                registry
                    .groups
                    .insert(name.clone(), Arc::new(Group::new(name, docs)));
            }
            Rule::func_def => {
                functions.push(Arc::new(parse_function(record, &registry)));
            }
            _ => continue,
        }
    }

    // Verify Output
    for func in functions {
        println!(
            "Function: {} (UID: {}, Group: {:?})",
            func.name,
            func.uid,
            func.group.as_ref().map(|g| &g.name)
        );
        println!("  Consumes: {:?}", func.consumes);
        println!("  Produces: {:?}", func.produces);
        println!("---");
    }
}

/// Extracts documentation and identifier from a definition statement.
fn parse_def(pair: pest::iterators::Pair<Rule>) -> (String, Option<String>) {
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
    let name = inner.next().unwrap().as_str().to_string();
    let doc_str = if docs.is_empty() {
        None
    } else {
        Some(docs.join("\n"))
    };

    (name, doc_str)
}

/// Parses a function contract, resolving its input arguments without parentheses.
fn parse_function(pair: pest::iterators::Pair<Rule>, registry: &SymbolRegistry) -> Function {
    let mut inner = pair.into_inner();
    let mut docs = Vec::new();
    let mut group = None;

    // 1. Process Documentation
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

    // 2. Process Optional Group Prefix
    if let Some(p) = inner.peek() {
        if p.as_rule() == Rule::ident {
            let group_name = inner.next().unwrap().as_str();
            group = registry.groups.get(group_name).cloned();
        }
    }

    let _kw = inner.next().unwrap(); // 'function'
    let name = inner.next().unwrap().as_str().to_string();

    // 3. Process Consumes (Inputs)
    // Grammar no longer uses '(' or ')'.
    let mut consumes = Vec::new();
    if let Some(p) = inner.peek() {
        if p.as_rule() == Rule::token_list {
            consumes = parse_token_list(inner.next().unwrap(), registry);
        }
    }

    // 4. Process Produces (Branching Outputs)
    let mut produces = Vec::new();
    if let Some(p) = inner.next() {
        for output_line in p.into_inner() {
            let list_pair = output_line
                .into_inner()
                .next()
                .expect("Expected token_list in output_line");
            produces.push(parse_token_list(list_pair, registry));
        }
    }

    let doc_content = if docs.is_empty() {
        None
    } else {
        Some(docs.join("\n"))
    };

    Function::new(name, doc_content, consumes, produces, group)
}

/// Converts a comma-separated list of symbols into model Tokens.
fn parse_token_list(pair: pest::iterators::Pair<Rule>, registry: &SymbolRegistry) -> Vec<Token> {
    let mut tokens = Vec::new();
    for token_pair in pair.into_inner() {
        let inner = token_pair.into_inner().next().unwrap();
        let (name, cardinality) = match inner.as_rule() {
            Rule::collection => (
                inner.into_inner().next().unwrap().as_str(),
                Cardinality::Collection,
            ),
            Rule::unitary => (inner.as_str(), Cardinality::Unitary),
            _ => unreachable!(),
        };

        let kind = registry
            .kinds
            .get(name)
            .cloned()
            .unwrap_or_else(|| Kind::Variable(Arc::new(Variable::new(name.to_string(), None))));

        tokens.push(Token::new(kind, cardinality));
    }
    tokens
}
