use pest_derive::Parser;
use std::collections::HashMap;
use std::sync::Arc;

// Import models and constructors
use crate::models::{Cardinality, Constant, Error, Function, Group, Kind, Token, Variable};

#[derive(Parser)]
#[grammar = "tect.pest"]
pub struct TectParser;

/// Local registry to track defined symbols during the parsing pass.
struct SymbolRegistry {
    /// Maps symbol names to their Logical Kind (which internally holds the Arc artifact).
    kinds: HashMap<String, Kind>,
    /// Maps group names to their logical Group objects.
    groups: HashMap<String, Arc<Group>>,
}

#[test]
fn main() {
    use pest::Parser;
    use std::fs;

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

    // Pass: Definitions and Function discovery
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

    // Output results for verification
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

/// Extracts documentation lines and the identifier from a definition rule.
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

/// Parses a function contract, resolving inputs and outputs against the registry.
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

    // 2. Process Group Prefix (Optional)
    if let Some(p) = inner.peek() {
        if p.as_rule() == Rule::ident {
            let group_name = inner.next().unwrap().as_str();
            group = registry.groups.get(group_name).cloned();
        }
    }

    let _kw = inner.next().unwrap();
    let name = inner.next().unwrap().as_str().to_string();

    // 3. Process Consumes (Inputs)
    let mut consumes = Vec::new();
    if let Some(p) = inner.peek() {
        if p.as_rule() == Rule::token_list {
            consumes = parse_token_list(inner.next().unwrap(), registry);
        }
    }

    // 4. Process Produces (Branching Outputs)
    let mut produces = Vec::new();
    if let Some(p) = inner.next() {
        // p is func_outputs
        for output_line in p.into_inner() {
            // output_line is (">" | "|") ~ token_list
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

    // Use constructor to ensure UID encapsulation
    Function::new(name, doc_content, consumes, produces, group)
}

/// Converts a Pest token_list into model Tokens, resolving Types from the registry.
fn parse_token_list(pair: pest::iterators::Pair<Rule>, registry: &SymbolRegistry) -> Vec<Token> {
    let mut tokens = Vec::new();
    for token_pair in pair.into_inner() {
        // token_pair is { collection | unitary }
        let inner = token_pair.into_inner().next().unwrap();
        let (name, cardinality) = match inner.as_rule() {
            Rule::collection => (
                inner.into_inner().next().unwrap().as_str(),
                Cardinality::Collection,
            ),
            Rule::unitary => (inner.as_str(), Cardinality::Unitary),
            _ => unreachable!(),
        };

        // Lookup Type in registry or fallback to a default Variable if not found.
        let kind = registry
            .kinds
            .get(name)
            .cloned()
            .unwrap_or_else(|| Kind::Variable(Arc::new(Variable::new(name.to_string(), None))));

        // Use constructor to ensure UID encapsulation
        tokens.push(Token::new(kind, cardinality));
    }
    tokens
}
