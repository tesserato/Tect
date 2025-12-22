#[cfg(test)]
mod tests {
    use crate::{Rule, TectAnalyzer, TectParser};
    use pest::Parser;

    #[test]
    fn test_parse_data_definition() {
        let input = "Data Credentials";
        let pair = TectParser::parse(Rule::program, input);
        assert!(pair.is_ok());
    }

    #[test]
    fn test_parse_function_union() {
        let input = "Function Login(Credentials) -> Session | AuthError";
        let pair = TectParser::parse(Rule::program, input);
        assert!(pair.is_ok());
    }

    #[test]
    fn test_parse_match_arms() {
        let input = r#"
            Match res {
                Session => { 
                    Break 
                }
                Error => { 
                    Log(e) 
                }
                _ => {
                    Break
                }
            }
        "#;
        let pair = TectParser::parse(Rule::program, input);
        assert!(pair.is_ok());
    }

    #[test]
    fn test_parse_for_loop() {
        let input = "For i in 0..10 { res = Work(i) }";
        let pair = TectParser::parse(Rule::program, input);
        assert!(pair.is_ok());
    }

    #[test]
    fn test_type_inference_from_function() {
        let input = "Data S\nFunction F(u)->S\nres = F(u)";
        let mut a = TectAnalyzer::new();
        a.analyze(input);
        assert_eq!(a.symbols.get("res").unwrap().detail, "S");
    }

    #[test]
    fn test_variable_instantiation_type() {
        let input = "u: Credentials";
        let mut a = TectAnalyzer::new();
        a.analyze(input);
        assert_eq!(a.symbols.get("u").unwrap().detail, "Credentials");
    }

    #[test]
    fn test_doc_comment_association() {
        let input = "# Doc 1\n# Doc 2\nData Credentials";
        let mut a = TectAnalyzer::new();
        a.analyze(input);
        let s = a.symbols.get("Credentials").unwrap();
        let docs = s.docs.as_ref().unwrap();
        assert!(docs.contains("Doc 1") && docs.contains("Doc 2"));
    }

    #[test]
    fn test_nested_variable_inference() {
        let input = "Data S\nFunction F(u)->S\nFor i in 0..3 { v = F(u) }";
        let mut a = TectAnalyzer::new();
        a.analyze(input);
        assert_eq!(a.symbols.get("v").unwrap().detail, "S");
    }

    #[test]
    fn test_unknown_function_assignment() {
        let input = "res = UnknownFunc(u)";
        let mut a = TectAnalyzer::new();
        a.analyze(input);
        assert_eq!(a.symbols.get("res").unwrap().detail, "Unknown");
    }

    #[test]
    fn test_strict_newline_doc_separation() {
        let input = "# Header\n\n# Doc\nData C";
        let mut a = TectAnalyzer::new();
        a.analyze(input);
        let docs = a.symbols.get("C").unwrap().docs.as_ref().unwrap();
        assert!(!docs.contains("Header") && docs.contains("Doc"));
    }

    #[test]
    fn test_naked_call_syntax() {
        let input = "Trigger(alert)";
        let pair = TectParser::parse(Rule::program, input);
        assert!(pair.is_ok());
    }
}
