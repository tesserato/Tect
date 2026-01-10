use crate::analyzer::{Rule, TectAnalyzer, TectParser};
use pest::Parser;
use std::path::PathBuf;

/// Verifies that data artifacts can be defined correctly.
#[test]
fn test_parse_data_definition() {
    let input = "constant Settings";
    let pair = TectParser::parse(Rule::program, input);
    assert!(pair.is_ok());
}

/// Verifies that functions parse correctly without parentheses.
#[test]
fn test_parse_function_no_parens() {
    let input = "function Login Credentials\n > Session";
    let pair = TectParser::parse(Rule::program, input);
    assert!(pair.is_ok());
}

/// Ensures leading documentation comments are captured and associated.
#[test]
fn test_doc_comment_association() {
    let input = "# Doc 1\n# Doc 2\nconstant Credentials";
    let mut a = TectAnalyzer::new();
    let structure = a.analyze(input, PathBuf::from("test.tect"));
    let s = structure.artifacts.get("Credentials").unwrap();

    let docs = match s {
        crate::models::Kind::Constant(c) => c.documentation.as_ref().unwrap(),
        _ => panic!("Expected constant artifact"),
    };
    assert!(docs.contains("Doc 1") && docs.contains("Doc 2"));
}

/// Validates that vertical whitespace separates documentation blocks.
#[test]
fn test_strict_newline_doc_separation() {
    let input = "# Header\n\n# Doc\nconstant C";
    let mut a = TectAnalyzer::new();
    let structure = a.analyze(input, PathBuf::from("test.tect"));
    let s = structure.artifacts.get("C").unwrap();

    let docs = match s {
        crate::models::Kind::Constant(c) => c.documentation.as_ref().unwrap(),
        _ => panic!("Expected constant artifact"),
    };
    assert!(!docs.contains("Header") && docs.contains("Doc"));
}

/// Verifies that lowercase identifiers are allowed (no strict PascalCase enforcement).
#[test]
fn test_lowercase_allowed() {
    let input = "constant credentials";
    let pair = TectParser::parse(Rule::program, input);
    assert!(pair.is_ok());
}