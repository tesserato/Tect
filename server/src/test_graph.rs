use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Write;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Eq, Hash)]
pub enum Cardinality {
    Unitary,
    Collection,
}

// #[derive(Debug, Serialize, Deserialize, Clone)]
// pub struct Type {
//     pub name: String,
//     pub documentation: Option<String>,
//     pub detail: Kind,
// }

#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq)]
pub enum Type {
    Variable {
        name: String,
        documentation: Option<String>,
    },
    Constant {
        name: String,
        documentation: Option<String>,
    },
    Error {
        name: String,
        documentation: Option<String>,
    },
}

// Tokens double as edges in the graph
#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq)]
pub struct Token {
    pub r#type: Type,
    pub cardinality: Cardinality,
    pub origin_function: Function,
    pub destination_function: Option<Function>,
}

impl Token {
    pub fn new(r#type: Type, cardinality: Cardinality, origin_function: Function) -> Self {
        Self {
            r#type,
            cardinality,
            origin_function,
            destination_function: None,
        }
    }
}

// Functions double as nodes in the graph
#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    pub documentation: Option<String>,
    // pub uid: u32,
    pub consumes: Vec<Token>,
    pub produces: Vec<Vec<Token>>,
    // pub is_artificial_graph_start: bool,
    // pub is_artificial_graph_end: bool,
    // pub is_artificial_error_termination: bool,
}

pub struct TokenPool {
    pub variables: Vec<Token>,
    pub errors: Vec<Token>,
    pub constants: HashSet<Token>,
    // pub consumed: Vec<Token>,
}

impl TokenPool {
    pub fn new(tokens: Vec<Token>) -> Self {
        let mut variables = Vec::new();
        let mut errors = Vec::new();
        let mut constants = HashSet::new();

        for token in tokens {
            match token.r#type {
                Type::Variable { .. } => variables.push(token),
                Type::Error { .. } => errors.push(token),
                Type::Constant { .. } => {
                    constants.insert(token);
                }
            }
        }
        Self {
            variables,
            errors,
            constants,
        }
    }

    pub fn add(&mut self, tokens: Vec<Token>) {
        for token in tokens {
            match token.r#type {
                Type::Variable { .. } => self.variables.push(token),
                Type::Error { .. } => self.errors.push(token),
                Type::Constant { .. } => {
                    self.constants.insert(token);
                }
            }
        }
    }

    pub fn consume(&mut self, tokens: Vec<Token>) -> bool {
        let mut edges = Vec::new();
        let mut triggered_expansion = false;

        // Python parity: Find ALL matches to create multiple edges if multiple origins exist
        let matching_indices: Vec<usize> = self
            .available
            .iter()
            .enumerate()
            .filter(|(_, t)| t.name == req.name)
            .map(|(i, _)| i)
            .collect();

        for &idx in &matching_indices {
            let matched = self.available[idx].clone();
            if matched.is_collection && !req.is_collection {
                triggered_expansion = true;
            }

            let mut edge = matched.clone();
            edge.destination_function_uid = Some(consumer_uid);
            edges.push(edge);

            self.consumed.push(matched.clone());

            // If mutable, we stop after the first removal to match Python's break
            if matched.is_mutable {
                self.available.remove(idx);
                break;
            }
        }
        (edges, triggered_expansion)
    }
}

pub struct FlowProcessor {
    uid_counter: u32,
}

impl FlowProcessor {
    pub fn new() -> Self {
        Self { uid_counter: 1 }
    }

    pub fn generate_function(
        &mut self,
        name: &str,
        consumes: Vec<(Token, Cardinality)>,
        produces: Vec<(Token, Cardinality)>,
        is_start: bool,
        is_end: bool,
        is_error: bool,
    ) -> Function {
        let uid = self.uid_counter;
        self.uid_counter += 1;
        Function {
            name: name.to_string(),
            uid,
            consumes: consumes
                .into_iter()
                .map(|(mut t, c)| {
                    t.is_collection = c == Cardinality::Collection;
                    t
                })
                .collect(),
            produces: produces
                .into_iter()
                .map(|(mut t, c)| {
                    t.is_collection = c == Cardinality::Collection;
                    t
                })
                .collect(),
            is_artificial_graph_start: is_start,
            is_artificial_graph_end: is_end,
            is_artificial_error_termination: is_error,
        }
    }

    pub fn process_flow(&mut self, pipeline: Vec<Function>) -> (Vec<Function>, Vec<Token>) {
        let mut pool = TokenPool::new();
        let mut nodes = Vec::new();
        let mut all_edges = Vec::new();

        let start_node = self.generate_function("Start", vec![], vec![], true, false, false);
        let end_node = self.generate_function("End", vec![], vec![], false, true, false);

        if let Some(first) = pipeline.first() {
            for req in &first.consumes {
                pool.add(req.clone(), start_node.uid, false);
            }
        }
        nodes.push(start_node);

        for mut func in pipeline {
            let mut func_is_expanded = false;
            for req in &func.consumes {
                let (edges, expanded) = pool.consume_requirement(req, func.uid);
                all_edges.extend(edges);
                if expanded {
                    func_is_expanded = true;
                }
            }
            let products = func.produces.clone();
            for prod in products {
                pool.add(prod, func.uid, func_is_expanded);
            }
            nodes.push(func);
        }

        let mut error_nodes: HashMap<String, Function> = HashMap::new();
        let leftovers: Vec<Token> = pool.available.drain(..).collect();

        for mut leftover in leftovers {
            // Skips terminal edge if this specific token was already used elsewhere
            if pool.consumed.contains(&leftover) {
                continue;
            }

            let target_uid = if leftover.name.contains("Error") {
                let name = leftover.name.clone();
                error_nodes
                    .entry(name.clone())
                    .or_insert_with(|| {
                        self.generate_function(&name, vec![], vec![], false, true, true)
                    })
                    .uid
            } else {
                end_node.uid
            };

            leftover.destination_function_uid = Some(target_uid);
            all_edges.push(leftover);
        }

        for (_, err_node) in error_nodes {
            nodes.push(err_node);
        }
        nodes.push(end_node);
        (nodes, all_edges)
    }
}

#[derive(Serialize)]
struct GraphExport {
    nodes: Vec<Function>,
    edges: Vec<Token>,
}

#[test]
fn main() -> std::io::Result<()> {

    // Define types (constants, variables, errors)
    let initial_command = Type::Variable {
        name: "InitialCommand".to_string(),
        documentation: Some("The initial command input from the CLI".to_string()),
    };
    let path_to_config = Type::Variable {
        name: "PathToConfig".to_string(),
        documentation: Some("The path to the configuration file".to_string()),
    };

    let settings = Type::Constant {
        name: "Settings".to_string(),
        documentation: Some("The loaded settings from the config file".to_string()),
    };

let templates = Type::Constant {
        name: "Templates".to_string(),
        documentation: Some("The registry of HTML templates used for rendering".to_string()),
    };

    let source_file = Type::Constant {
        name: "SourceFile".to_string(),
        documentation: Some("A raw input file found in the source directory".to_string()),
    };

    let article = Type::Variable {
        name: "Article".to_string(),
        documentation: Some("The processed data structure containing markdown content and metadata".to_string()),
    };

    let html = Type::Variable {
        name: "HTML".to_string(),
        documentation: Some("The final rendered HTML string ready to be written to disk".to_string()),
    };

    let fs_error = Type::Error {
        name: "FileSystemError".to_string(),
        documentation: Some("Triggered when a file cannot be read from or written to the disk".to_string()),
    };

    let success = Type::Variable {
        name: "SuccessReport".to_string(),
        documentation: Some("A final summary of the operations performed during the run".to_string()),
    };

    // define functions

    let process_cli = Function {
        name: "ProcessCLI".to_string(),
        documentation: Some("Processes command-line input".to_string()),
        consumes: vec![Token::new(initial_command.clone(), Cardinality::Unitary, /* origin_function */)],
        produces: vec![
            vec![Token::new(settings.clone(), Cardinality::Unitary, /* origin_function */)],
            vec![Token::new(path_to_config.clone(), Cardinality::Unitary, /* origin_function */)],
        ],
    };

    let pipeline = vec![
        engine.generate_function(
            "ProcessCLI",
            vec![(initial_command, Cardinality::Unitary)],
            vec![
                (settings.clone(), Cardinality::Unitary),
                (path_to_config.clone(), Cardinality::Unitary),
            ],
            false,
            false,
            false,
        ),
        engine.generate_function(
            "LoadConfig",
            vec![(path_to_config, Cardinality::Unitary)],
            vec![(settings.clone(), Cardinality::Unitary)],
            false,
            false,
            false,
        ),
        engine.generate_function(
            "LoadTemplates",
            vec![(settings.clone(), Cardinality::Unitary)],
            vec![(templates.clone(), Cardinality::Unitary)],
            false,
            false,
            false,
        ),
        engine.generate_function(
            "ScanFS",
            vec![(settings.clone(), Cardinality::Unitary)],
            vec![
                (source_file.clone(), Cardinality::Collection),
                (fs_error.clone(), Cardinality::Collection),
            ],
            false,
            false,
            false,
        ),
        engine.generate_function(
            "ParseMarkdown",
            vec![(source_file, Cardinality::Unitary)],
            vec![
                (article.clone(), Cardinality::Unitary),
                (fs_error.clone(), Cardinality::Unitary),
            ],
            false,
            false,
            false,
        ),
        engine.generate_function(
            "RenderHTML",
            vec![
                (article, Cardinality::Unitary),
                (templates, Cardinality::Unitary),
                (settings, Cardinality::Unitary),
            ],
            vec![(html.clone(), Cardinality::Unitary)],
            false,
            false,
            false,
        ),
        engine.generate_function(
            "WriteToDisk",
            vec![(html, Cardinality::Collection)],
            vec![
                (success, Cardinality::Unitary),
                (fs_error, Cardinality::Collection),
            ],
            false,
            false,
            false,
        ),
    ];

    // Serialization with 4-space indentation
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    let (nodes, edges) = engine.process_flow(pipeline);

    (GraphExport { nodes, edges }).serialize(&mut ser).unwrap();
    let json_data = String::from_utf8(buf).unwrap();
    let mut file = File::create("../experiments/architecture.json")?;
    file.write_all(json_data.as_bytes())?;
    Ok(())
}
