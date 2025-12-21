use pest::Parser;
use pest_derive::Parser;
use std::collections::{HashMap, HashSet};

#[derive(Parser)]
#[grammar = "src/tect.pest"]
pub struct TectParser;

#[derive(Debug, Clone)]
enum TectType {
    Data(String),
    Error(String),
    Union(Vec<String>), // e.g., Token | AuthFail
}

#[derive(Debug)]
struct FunctionSig {
    input: String,
    outputs: Vec<String>,
}

struct Analyzer {
    data_types: HashSet<String>,
    error_types: HashSet<String>,
    functions: HashMap<String, FunctionSig>,
    instances: HashMap<String, String>, // var_name -> type_name
}

impl Analyzer {
    fn new() -> Self {
        Self {
            data_types: HashSet::new(),
            error_types: HashSet::new(),
            functions: HashMap::new(),
            instances: HashMap::new(),
        }
    }

    fn analyze(&mut self, input: &str) {
        let pairs = TectParser::parse(Rule::program, input).expect("Parse failed");

        for pair in pairs.into_iter().next().unwrap().into_inner() {
            match pair.as_rule() {
                Rule::data_def => {
                    let name = pair.into_inner().next().unwrap().as_str();
                    self.data_types.insert(name.to_string());
                    println!("✔ Defined Data: {}", name);
                }
                Rule::error_def => {
                    let name = pair.into_inner().next().unwrap().as_str();
                    self.error_types.insert(name.to_string());
                    println!("✔ Defined Error: {}", name);
                }
                Rule::func_def => {
                    let mut inner = pair.into_inner();
                    let name = inner.next().unwrap().as_str();
                    let input_type = inner.next().unwrap().as_str();
                    let outputs = inner
                        .next()
                        .unwrap()
                        .into_inner()
                        .map(|id| id.as_str().to_string())
                        .collect::<Vec<_>>();

                    self.functions.insert(
                        name.to_string(),
                        FunctionSig {
                            input: input_type.to_string(),
                            outputs,
                        },
                    );
                    println!("✔ Defined Function: {}", name);
                }
                Rule::instantiation => {
                    let mut inner = pair.into_inner();
                    let var_name = inner.next().unwrap().as_str();
                    let type_name = inner.next().unwrap().as_str();
                    self.instances
                        .insert(var_name.to_string(), type_name.to_string());
                }
                Rule::assignment => {
                    let mut inner = pair.into_inner();
                    let target_var = inner.next().unwrap().as_str();
                    let func_name = inner.next().unwrap().as_str();
                    let arg_var = inner.next().unwrap().as_str();

                    self.check_assignment(target_var, func_name, arg_var);
                }
                _ => {}
            }
        }
    }

    fn check_assignment(&self, target: &str, func_name: &str, arg: &str) {
        let sig = self.functions.get(func_name).expect("Function not defined");
        let arg_type = self.instances.get(arg).expect("Variable not defined");

        // 1. Type Match Check
        if sig.input != *arg_type {
            println!(
                "❌ ARCHITECTURAL ERROR: Function '{}' expects '{}', but '{}' is '{}'",
                func_name, sig.input, arg, arg_type
            );
        }

        // 2. Error Path Analysis
        let has_error = sig.outputs.iter().any(|out| self.error_types.contains(out));
        if has_error {
            println!(
                "⚠️  WARNING: Assignment to '{}' contains unhandled Error paths: {:?}",
                target,
                sig.outputs
                    .iter()
                    .filter(|o| self.error_types.contains(*o))
                    .collect::<Vec<_>>()
            );
        } else {
            println!("✔ Assignment to '{}' is safe (no error branches).", target);
        }
    }

    fn generate_mermaid(&self) -> String {
        let mut chart = String::from("graph TD\n");
        for (name, sig) in &self.functions {
            for out in &sig.outputs {
                let color = if self.error_types.contains(out) {
                    ":::error"
                } else {
                    ""
                };
                chart.push_str(&format!("  {}([{}]) --> {}{}\n", name, name, out, color));
            }
        }
        chart
    }
}

fn main() {
    let code = r#"
        Data Credentials
        Data Token
        Error AuthFail

        Function Login(Credentials) -> Token|AuthFail

        creds_john: Credentials
        res = Login(creds_john)
    "#;

    let mut analyzer = Analyzer::new();
    analyzer.analyze(code);

    println!("\n--- Generated Diagram (Mermaid) ---");
    println!("{}", analyzer.generate_mermaid());
}
