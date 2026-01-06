#[cfg(test)]
mod tests {
    use crate::analyzer::{Rule, TectAnalyzer, TectParser};
    use pest::Parser;

    /// Tests the formal parsing of basic data definitions.
    #[test]
    fn test_parse_data_definition() {
        let input = "constant Settings";
        let pair = TectParser::parse(Rule::program, input);
        assert!(pair.is_ok());
    }

    /// Tests function signature parsing without parentheses.
    #[test]
    fn test_parse_function_no_parens() {
        let input = "function Login Credentials\n > Session";
        let pair = TectParser::parse(Rule::program, input);
        assert!(pair.is_ok());
    }

    /// Verifies documentation comments are correctly associated with defined entities.
    #[test]
    fn test_doc_comment_association() {
        let input = "# Doc 1\n# Doc 2\nconstant Credentials";
        let mut a = TectAnalyzer::new();
        let structure = a.analyze(input).unwrap();
        let s = structure.artifacts.get("Credentials").unwrap();

        let docs = match s {
            crate::models::Kind::Constant(c) => c.documentation.as_ref().unwrap(),
            _ => panic!("Expected constant"),
        };
        assert!(docs.contains("Doc 1") && docs.contains("Doc 2"));
    }

    /// Validates multi-line comment separation logic.
    #[test]
    fn test_strict_newline_doc_separation() {
        let input = "# Header\n\n# Doc\nconstant C";
        let mut a = TectAnalyzer::new();
        let structure = a.analyze(input).unwrap();
        let s = structure.artifacts.get("C").unwrap();

        let docs = match s {
            crate::models::Kind::Constant(c) => c.documentation.as_ref().unwrap(),
            _ => panic!("Expected constant"),
        };
        assert!(!docs.contains("Header") && docs.contains("Doc"));
    }

    /// Enforces Tect's casing rules (PascalCase for identifiers).
    #[test]
    fn test_strict_casing_failure() {
        // lower case is not allowed for artifact names
        let input = "constant credentials";
        let pair = TectParser::parse(Rule::program, input);
        assert!(pair.is_err());
    }
}
