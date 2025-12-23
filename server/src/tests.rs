#[cfg(test)]
mod tests {
    use crate::analyzer::{Rule, TectAnalyzer, TectParser};
    use pest::Parser;

    /// Tests the formal parsing of basic data definitions.
    #[test]
    fn test_parse_data_definition() {
        let input = "data Credentials";
        let pair = TectParser::parse(Rule::program, input);
        assert!(pair.is_ok());
    }

    /// Tests function signature parsing including type unions.
    #[test]
    fn test_parse_function_union() {
        let input = "function Login(Credentials) -> Session | AuthError";
        let pair = TectParser::parse(Rule::program, input);
        assert!(pair.is_ok());
    }

    /// Tests architectural branching (match) syntax.
    #[test]
    fn test_parse_match_arms() {
        let input = r#"
            match res {
                Session => { 
                    break 
                }
                Error => { 
                    Log(e) 
                }
                _ => {
                    break
                }
            }
        "#;
        let pair = TectParser::parse(Rule::program, input);
        assert!(pair.is_ok());
    }

    /// Tests loop construct parsing.
    #[test]
    fn test_parse_for_loop() {
        let input = "for i in 0..10 { res = Work(i) }";
        let pair = TectParser::parse(Rule::program, input);
        assert!(pair.is_ok());
    }

    /// Verifies that variable types are correctly inferred from function return signatures.
    #[test]
    fn test_type_inference_from_function() {
        let input = "data S\nfunction F(U)->S\nres = F(u)";
        let mut a = TectAnalyzer::new();
        let _ = a.analyze(input);
        assert_eq!(a.symbols.get("res").unwrap().detail, "S");
    }

    /// Verifies explicit variable instantiation metadata.
    #[test]
    fn test_variable_instantiation_type() {
        let input = "u: Credentials";
        let mut a = TectAnalyzer::new();
        let _ = a.analyze(input);
        assert_eq!(a.symbols.get("u").unwrap().detail, "Credentials");
    }

    /// Ensures documentation comments are correctly associated with defined entities.
    #[test]
    fn test_doc_comment_association() {
        let input = "# Doc 1\n# Doc 2\ndata Credentials";
        let mut a = TectAnalyzer::new();
        let _ = a.analyze(input);
        let s = a.symbols.get("Credentials").unwrap();
        let docs = s.docs.as_ref().unwrap();
        assert!(docs.contains("Doc 1") && docs.contains("Doc 2"));
    }

    /// Tests type inference within nested architectural flows.
    #[test]
    fn test_nested_variable_inference() {
        let input = "data S\nfunction F(U)->S\nfor i in 0..3 { v = F(u) }";
        let mut a = TectAnalyzer::new();
        let _ = a.analyze(input);
        assert_eq!(a.symbols.get("v").unwrap().detail, "S");
    }

    /// Verifies fallback behavior when an undefined function is called.
    #[test]
    fn test_unknown_function_assignment() {
        let input = "res = UnknownFunc(u)";
        let mut a = TectAnalyzer::new();
        let _ = a.analyze(input);
        assert_eq!(a.symbols.get("res").unwrap().detail, "Unknown");
    }

    /// Validates multi-line comment separation logic.
    #[test]
    fn test_strict_newline_doc_separation() {
        let input = "# Header\n\n# Doc\ndata C";
        let mut a = TectAnalyzer::new();
        let _ = a.analyze(input);
        let docs = a.symbols.get("C").unwrap().docs.as_ref().unwrap();
        assert!(!docs.contains("Header") && docs.contains("Doc"));
    }

    /// Verifies side-effect call syntax (no assignment).
    #[test]
    fn test_naked_call_syntax() {
        let input = "Trigger(alert)";
        let pair = TectParser::parse(Rule::program, input);
        assert!(pair.is_ok());
    }

    /// Enforces Tect's hard casing rules (Upper for types, Lower for logic).
    #[test]
    fn test_strict_casing_failure() {
        let input = "data credentials";
        let pair = TectParser::parse(Rule::program, input);
        assert!(pair.is_err());

        let input = "User_Input: Credentials";
        let pair = TectParser::parse(Rule::program, input);
        assert!(pair.is_err());
    }
}
