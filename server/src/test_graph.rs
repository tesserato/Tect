use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Write;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Eq, Hash)]
pub enum Cardinality {
    Unitary,
    Collection,
}

#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq)]
pub struct Variable {
    name: String,
    documentation: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq)]
pub struct Constant {
    name: String,
    documentation: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq)]
pub struct Error {
    name: String,
    documentation: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq)]
pub enum Kind {
    Variable(Variable),
    Constant(Constant),
    Error(Error),
}

#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq)]
pub struct Group {
    name: String,
    documentation: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    pub documentation: Option<String>,
    pub consumes: Vec<Token>,
    pub produces: Vec<Vec<Token>>,
}

// Mutable. References previous constant structs.
#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq)]
pub struct Token {
    pub kind: Arc<Kind>,
    pub cardinality: Cardinality,
    pub group: Option<Arc<Group>>,
}

impl Token {
    pub fn new(kind: Arc<Kind>, cardinality: Cardinality, group: Option<Arc<Group>>) -> Self {
        Self {
            kind,
            cardinality,
            group,
        }
    }
    pub fn compare(&self, other: &Self) -> bool {
        if self.kind == other.kind {
            true
        } else {
            false
        }
    }
}

// Graph components
#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq)]
pub struct Node {
    pub uid: u32,
    pub function: &Function,
    pub is_artificial_graph_start: bool,
    pub is_artificial_graph_end: bool,
    pub is_artificial_error_termination: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq)]
pub struct Edge {
    pub origin_function: &Function,
    pub destination_function: &Function,
    pub token: Token,
}

pub struct TokenPool {
    pub variables: Vec<Token>,
    pub errors: Vec<Token>,
    pub constants: HashSet<Token>,
    pub token_to_initial_node: HashMap<Token, Node>,
}

impl TokenPool {
    pub fn new(tokens: Vec<Token>, initial_node: Node) -> Self {
        let mut variables = Vec::new();
        let mut errors = Vec::new();
        let mut constants = HashSet::new();
        let mut token_to_initial_node = HashMap::new();

        for token in tokens {
            token_to_initial_node.insert(token.clone(), initial_node.clone());
            match token.kind {
                Kind::Variable { .. } => variables.push(token),
                Kind::Error { .. } => errors.push(token),
                Kind::Constant { .. } => {
                    constants.insert(token);
                }
            }
        }
        Self {
            variables,
            errors,
            constants,
            token_to_initial_node,
        }
    }

    pub fn add(&mut self, tokens: Vec<Token>, initial_node: Node) {
        for token in tokens {
            self.token_to_initial_node
                .insert(token.clone(), initial_node.clone());
            match token.kind {
                Kind::Variable { .. } => self.variables.push(token),
                Kind::Error { .. } => self.errors.push(token),
                Kind::Constant { .. } => {
                    self.constants.insert(token);
                }
            }
        }
    }

    pub fn consume(&mut self, tokens: Vec<Token>, destination_function: Function) -> Vec<Edge> {
        let mut edges = Vec::new();

        for token in tokens {
            match token.kind {
                Kind::Variable { .. } => {
                    match self.variables.iter().position(|t| t.compare(&token)) {
                        Some(index) => {
                            let consumed_variable = self.variables.remove(index);
                            if let Some(node) = self.token_to_initial_node.get(&token) {
                                edges.push(Edge {
                                    origin_function: node.function.clone(),
                                    destination_function: destination_function.clone(),
                                    token: consumed_variable,
                                });
                            }
                        }
                        None => {}
                    }
                }
                Kind::Error { .. } => match self.errors.iter().position(|t| t.compare(&token)) {
                    Some(index) => {
                        let consumed_error = self.errors.remove(index);
                        if let Some(node) = self.token_to_initial_node.get(&token) {
                            edges.push(Edge {
                                origin_function: node.function.clone(),
                                destination_function: destination_function.clone(),
                                token: consumed_error,
                            });
                        }
                    }
                    None => {}
                },
                Kind::Constant { .. } => {
                    // Iterate through every available constant in the state
                    for constant_token in &self.constants {
                        // Test the incoming token against the current constant token
                        if constant_token.compare(&token) {
                            // If a match is found, attempt to find the initial node for the matching constant
                            if let Some(node) = self.token_to_initial_node.get(constant_token) {
                                edges.push(Edge {
                                    origin_function: node.function.clone(),
                                    destination_function: destination_function.clone(),
                                    token: constant_token.clone(),
                                });
                            }
                        }
                    }
                }
            }
        }
        edges
    }
}

pub struct Flow {
    uid_counter: u32,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub pools: Vec<TokenPool>,
}

// impl Flow {
//     pub fn new(functions: Vec<Function>) -> Self {
//         let mut nodes = Vec::new();
//         let mut edges = Vec::new();
//         let mut pools = Vec::new();

//         let initial_node = Node {
//             uid: 0,
//             function: Function {

//         for function in functions {
//             let node = Node {

//                 function: function.clone(),
//                 is_artificial_graph_start: false,
//                 is_artificial_graph_end: false,
//                 is_artificial_error_termination: false,
//             };
//             nodes.push(node);
//         }

//         Self {
//             uid_counter: 0,
//             nodes: Vec::new(),
//             edges: Vec::new(),
//             pools: Vec::new(),
//         }
//     }

// }

#[derive(Serialize)]
struct GraphExport {
    nodes: Vec<Function>,
    edges: Vec<Token>,
}

#[test]
fn main() -> std::io::Result<()> {
    // Define types (constants, variables, errors)
    let initial_command = Variable {
        name: "InitialCommand".to_string(),
        documentation: Some("The initial command input from the CLI".to_string()),
    };
    let path_to_config = Variable {
        name: "PathToConfig".to_string(),
        documentation: Some("The path to the configuration file".to_string()),
    };

    let settings = Constant {
        name: "Settings".to_string(),
        documentation: Some("The loaded settings from the config file".to_string()),
    };

    let templates = Constant {
        name: "Templates".to_string(),
        documentation: Some("The registry of HTML templates used for rendering".to_string()),
    };

    let source_file = Constant {
        name: "SourceFile".to_string(),
        documentation: Some("A raw input file found in the source directory".to_string()),
    };

    let article = Variable {
        name: "Article".to_string(),
        documentation: Some(
            "The processed data structure containing markdown content and metadata".to_string(),
        ),
    };

    let html = Variable {
        name: "HTML".to_string(),
        documentation: Some(
            "The final rendered HTML string ready to be written to disk".to_string(),
        ),
    };

    let fs_error = Error {
        name: "FileSystemError".to_string(),
        documentation: Some(
            "Triggered when a file cannot be read from or written to the disk".to_string(),
        ),
    };

    let success = Variable {
        name: "SuccessReport".to_string(),
        documentation: Some(
            "A final summary of the operations performed during the run".to_string(),
        ),
    };

    // define functions

    let process_cli = Function {
        name: "ProcessCLI".to_string(),
        documentation: Some("Processes command-line input".to_string()),
        consumes: vec![Token::new(
            Kind::Variable(initial_command.clone()),
            Cardinality::Unitary,
        )],
        produces: vec![
            vec![Token::new(
                Kind::Constant(settings.clone()),
                Cardinality::Unitary,
            )],
            vec![Token::new(
                Kind::Variable(path_to_config.clone()),
                Cardinality::Unitary,
            )],
        ],
    };

    let load_config = Function {
        name: "LoadConfig".to_string(),
        documentation: Some("Loads configuration from a file".to_string()),
        consumes: vec![Token::new(
            Kind::Variable(path_to_config.clone()),
            Cardinality::Unitary,
        )],
        produces: vec![vec![Token::new(
            Kind::Constant(settings.clone()),
            Cardinality::Unitary,
        )]],
    };
    let load_templates = Function {
        name: "LoadTemplates".to_string(),
        documentation: Some("Loads HTML templates based on settings".to_string()),
        consumes: vec![Token::new(
            Kind::Constant(settings.clone()),
            Cardinality::Unitary,
        )],
        produces: vec![vec![Token::new(
            Kind::Constant(templates.clone()),
            Cardinality::Unitary,
        )]],
    };
    let scan_fs = Function {
        name: "ScanFS".to_string(),
        documentation: Some("Scans the filesystem for source files".to_string()),
        consumes: vec![Token::new(
            Kind::Constant(settings.clone()),
            Cardinality::Unitary,
        )],
        produces: vec![
            vec![Token::new(
                Kind::Constant(source_file.clone()),
                Cardinality::Collection,
            )],
            vec![Token::new(
                Kind::Error(fs_error.clone()),
                Cardinality::Collection,
            )],
        ],
    };

    let parse_markdown = Function {
        name: "ParseMarkdown".to_string(),
        documentation: Some("Parses markdown files into article structures".to_string()),
        consumes: vec![Token::new(
            Kind::Constant(source_file.clone()),
            Cardinality::Unitary,
        )],
        produces: vec![
            vec![Token::new(
                Kind::Variable(article.clone()),
                Cardinality::Unitary,
            )],
            vec![Token::new(
                Kind::Error(fs_error.clone()),
                Cardinality::Unitary,
            )],
        ],
    };
    let render_html = Function {
        name: "RenderHTML".to_string(),
        documentation: Some("Renders articles into HTML using templates".to_string()),
        consumes: vec![
            Token::new(Kind::Variable(article.clone()), Cardinality::Unitary),
            Token::new(Kind::Constant(templates.clone()), Cardinality::Unitary),
            Token::new(Kind::Constant(settings.clone()), Cardinality::Unitary),
        ],
        produces: vec![vec![Token::new(
            Kind::Variable(html.clone()),
            Cardinality::Unitary,
        )]],
    };
    let write_to_disk = Function {
        name: "WriteToDisk".to_string(),
        documentation: Some("Writes HTML files to disk".to_string()),
        consumes: vec![Token::new(
            Kind::Variable(html.clone()),
            Cardinality::Unitary,
        )],
        produces: vec![
            vec![Token::new(
                Kind::Variable(success.clone()),
                Cardinality::Unitary,
            )],
            vec![Token::new(
                Kind::Error(fs_error.clone()),
                Cardinality::Unitary,
            )],
        ],
    };

    let pipeline = vec![
        process_cli,
        load_config,
        load_templates,
        scan_fs,
        parse_markdown,
        render_html,
        write_to_disk,
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
