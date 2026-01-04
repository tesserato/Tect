use pest::Parser;
use pest_derive::Parser;
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;

// Import your existing models
use crate::models::{Cardinality, Constant, Error, Function, Group, Kind, Token, Variable};

#[derive(Parser)]
#[grammar = "tect.pest"]
pub struct TectParser;

struct SymbolRegistry {
    kinds: HashMap<String, Arc<Kind>>,
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

    for record in program.into_inner() {
        match record.as_rule() {
            Rule::const_def => {
                let (name, docs) = parse_def(record);
                registry.kinds.insert(
                    name.clone(),
                    Arc::new(Kind::Constant(Arc::new(Constant {
                        name,
                        documentation: docs,
                    }))),
                );
            }
            Rule::var_def => {
                let (name, docs) = parse_def(record);
                registry.kinds.insert(
                    name.clone(),
                    Arc::new(Kind::Variable(Arc::new(Variable {
                        name,
                        documentation: docs,
                    }))),
                );
            }
            Rule::err_def => {
                let (name, docs) = parse_def(record);
                registry.kinds.insert(
                    name.clone(),
                    Arc::new(Kind::Error(Arc::new(Error {
                        name,
                        documentation: docs,
                    }))),
                );
            }
            Rule::group_def => {
                let (name, docs) = parse_def(record);
                registry.groups.insert(
                    name.clone(),
                    Arc::new(Group {
                        name,
                        documentation: docs,
                    }),
                );
            }
            Rule::func_def => {
                functions.push(Arc::new(parse_function(record, &registry)));
            }
            _ => continue,
        }
    }

    for func in functions {
        println!(
            "Function: {} (Group: {:?})",
            func.name,
            func.group.as_ref().map(|g| &g.name)
        );
        println!("  Consumes: {:?}", func.consumes);
        println!("  Produces: {:?}", func.produces);
        println!("---");
    }
}

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

fn parse_function(pair: pest::iterators::Pair<Rule>, registry: &SymbolRegistry) -> Function {
    let mut inner = pair.into_inner();
    let mut docs = Vec::new();
    let mut group = None;

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

    if let Some(p) = inner.peek() {
        if p.as_rule() == Rule::ident {
            let group_name = inner.next().unwrap().as_str();
            group = registry.groups.get(group_name).cloned();
        }
    }

    let _kw = inner.next().unwrap();
    let name = inner.next().unwrap().as_str().to_string();

    let mut consumes = Vec::new();
    if let Some(p) = inner.peek() {
        if p.as_rule() == Rule::token_list {
            consumes = parse_token_list(inner.next().unwrap(), registry);
        }
    }

    let mut produces = Vec::new();
    if let Some(p) = inner.next() {
        for output_line in p.into_inner() {
            // Corrected: Literals like '>' are not in the iterator.
            // The first item in output_line's inner is the token_list.
            let list_pair = output_line
                .into_inner()
                .next()
                .expect("Expected token_list in output_line");
            produces.push(parse_token_list(list_pair, registry));
        }
    }

    Function {
        name,
        documentation: if docs.is_empty() {
            None
        } else {
            Some(docs.join("\n"))
        },
        consumes,
        produces,
        group,
    }
}

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

        let kind = registry.kinds.get(name).cloned().unwrap_or_else(|| {
            Arc::new(Kind::Variable(Arc::new(Variable {
                name: name.to_string(),
                documentation: None,
            })))
        });

        tokens.push(Token::new(kind, cardinality));
    }
    tokens
}
