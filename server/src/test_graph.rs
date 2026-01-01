use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Write;
use std::sync::atomic::{AtomicU32, Ordering};
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
    Variable(Arc<Variable>),
    Constant(Arc<Constant>),
    Error(Arc<Error>),
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

static TOKEN_ID_COUNTER: AtomicU32 = AtomicU32::new(0);

// Mutable. References previous constant structs.
#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq)]
pub struct Token {
    pub uid: u32,
    pub kind: Arc<Kind>,
    pub cardinality: Cardinality,
    pub group: Option<Arc<Group>>,
}

impl Token {
    pub fn new(kind: Arc<Kind>, cardinality: Cardinality, group: Option<Arc<Group>>) -> Self {
        Self {
            uid: TOKEN_ID_COUNTER.fetch_add(1, Ordering::SeqCst),
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
    pub function: Arc<Function>,
    pub is_artificial_graph_start: bool,
    pub is_artificial_graph_end: bool,
    pub is_artificial_error_termination: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq)]
pub struct Edge {
    pub origin_function: Arc<Function>,
    pub destination_function: Arc<Function>,
    pub token: Token,
}

pub enum Consumed {
    AllTokens(Vec<Edge>),
    SomeTokens(Vec<Token>),
}

#[derive(Clone)]
pub struct TokenPool {
    pub variables: Vec<Token>,
    pub errors: Vec<Token>,
    pub constants: Vec<Token>,
    pub token_to_initial_node: HashMap<Token, Arc<Node>>,
}

impl TokenPool {
    pub fn log_contents(&self) {
        println!("\n=== CURRENT TOKEN POOL STATE ===");

        self.log_sub_pool("Variables", &self.variables);
        self.log_sub_pool("Constants", &self.constants);
        self.log_sub_pool("Errors", &self.errors);

        println!("=================================\n");
    }

    fn log_sub_pool(&self, label: &str, tokens: &[Token]) {
        println!(" {}:", label);
        if tokens.is_empty() {
            println!("   (empty)");
            return;
        }

        for token in tokens {
            // Get the producer node name from the hashmap
            let producer_name = self
                .token_to_initial_node
                .get(token)
                .map(|node| node.function.name.as_str())
                .unwrap_or("Unknown Source");

            // Extract the specific name from the Kind enum
            let kind_name = match &*token.kind {
                Kind::Variable(v) => &v.name,
                Kind::Constant(c) => &c.name,
                Kind::Error(e) => &e.name,
            };

            println!(
                "   - [{:?}] {} (from: {})",
                token.cardinality, kind_name, producer_name
            );
        }
    }

    pub fn new(tokens: Vec<Token>, initial_node: Arc<Node>) -> Self {
        let mut variables = Vec::new();
        let mut errors = Vec::new();
        let mut constants = Vec::new();
        let mut token_to_initial_node = HashMap::new();

        for token in tokens {
            token_to_initial_node.insert(token.clone(), initial_node.clone());
            match &*token.kind {
                Kind::Variable(..) => variables.push(token),
                Kind::Error(..) => errors.push(token),
                Kind::Constant(..) => constants.push(token),
            };
        }
        Self {
            variables,
            errors,
            constants,
            token_to_initial_node,
        }
    }

    pub fn produce(&mut self, tokens: Vec<Token>, initial_node: Arc<Node>) {
        for token in tokens {
            self.token_to_initial_node
                .insert(token.clone(), initial_node.clone());
            match &*token.kind {
                Kind::Variable(..) => self.variables.push(token),
                Kind::Error(..) => self.errors.push(token),
                Kind::Constant(..) => self.constants.push(token),
            };
        }
    }

    pub fn try_to_consume(&mut self, tokens: Vec<Token>, destination_node: Arc<Node>) -> Consumed {
        let mut edges = Vec::new();
        let mut consumed_tokens: Vec<Token> = Vec::new();
        for requested_token in &tokens {
            match &*requested_token.kind {
                Kind::Variable(..) => {
                    for variable_token in &self.variables {
                        if variable_token.compare(&requested_token)
                            && !consumed_tokens.contains(variable_token)
                        {
                            if let Some(node) = self.token_to_initial_node.get(variable_token) {
                                edges.push(Edge {
                                    origin_function: node.function.clone(),
                                    destination_function: destination_node.function.clone(),
                                    token: variable_token.clone(),
                                });
                                consumed_tokens.push(variable_token.clone());
                                break;
                            }
                        }
                    }
                }

                Kind::Error(..) => {
                    for error_token in &self.errors {
                        if error_token.compare(&requested_token) && !consumed_tokens.contains(error_token) {
                            if let Some(node) = self.token_to_initial_node.get(error_token) {
                                edges.push(Edge {
                                    origin_function: node.function.clone(),
                                    destination_function: destination_node.function.clone(),
                                    token: error_token.clone(),
                                });
                                consumed_tokens.push(error_token.clone());
                                break;
                            }
                        }
                    }
                }
                Kind::Constant(..) => {
                    for constant_token in &self.constants {
                        if constant_token.compare(&requested_token)
                            && !consumed_tokens.contains(constant_token)
                        {
                            if let Some(node) = self.token_to_initial_node.get(constant_token) {
                                edges.push(Edge {
                                    origin_function: node.function.clone(),
                                    destination_function: destination_node.function.clone(),
                                    token: constant_token.clone(),
                                });
                                consumed_tokens.push(constant_token.clone());
                                break;
                            }
                        }
                    }
                }
            }
        }
        let unconsumed_tokens: Vec<Token> = tokens
            .iter()
            .filter(|req_token| {
                // A token is unconsumed if the consumed_tokens list
                // doesn't contain anything that "matches" (via Kind)
                !consumed_tokens.iter().any(|c| c.compare(req_token))
            })
            .cloned()
            .collect();
        if unconsumed_tokens.is_empty() {
            print!(
                "All tokens consumed for function: {}\n",
                destination_node.function.name
            );
            &self.variables.retain(|t| !consumed_tokens.contains(t));
            &self.errors.retain(|t| !consumed_tokens.contains(t));
            return Consumed::AllTokens(edges);
        } else {
            return Consumed::SomeTokens(unconsumed_tokens);
        }
    }
}

pub struct Flow {
    uid_counter: u32,
    pub nodes: Vec<Arc<Node>>,
    pub edges: Vec<Edge>,
    pub pools: Vec<TokenPool>,
}

impl Flow {
    pub fn new() -> Self {
        Self {
            uid_counter: 0,
            nodes: Vec::new(),
            edges: Vec::new(),
            pools: Vec::new(),
        }
    }
    pub fn process_flow(&mut self, functions: &Vec<Arc<Function>>) -> (Vec<Arc<Node>>, Vec<Edge>) {
        let initial_node = Arc::new(Node {
            uid: 0,
            function: Arc::new(Function {
                name: "InitialNode".to_string(),
                documentation: Some("Artificial initial node".to_string()),
                consumes: vec![],
                produces: vec![],
            }),
            is_artificial_graph_start: true,
            is_artificial_graph_end: false,
            is_artificial_error_termination: false,
        });
        self.nodes.push(initial_node.clone());

        self.pools.push(TokenPool::new(
            functions.first().unwrap().consumes.clone(),
            initial_node.clone(),
        ));

        self.pools.first_mut().unwrap().log_contents();
        for function in functions {
            self.uid_counter += 1;
            let node = Arc::new(Node {
                uid: self.uid_counter,
                function: function.clone(),
                is_artificial_graph_start: false,
                is_artificial_graph_end: false,
                is_artificial_error_termination: false,
            });
            self.nodes.push(node.clone());

            let mut new_pools: Vec<TokenPool> = Vec::new();
            for pool in &mut self.pools {
                match pool.try_to_consume(function.consumes.clone(), node.clone()) {
                    Consumed::AllTokens(new_edges) => {
                        self.edges.extend(new_edges.clone());
                        for produced_tokens in &function.produces {
                            let mut new_pool = pool.clone();
                            new_pool.produce(produced_tokens.clone(), node.clone());
                            new_pools.push(new_pool);
                        }
                    }
                    Consumed::SomeTokens(unconsumed_tokens) => {
                        new_pools.push(pool.clone());
                    }
                }
            }
            self.pools = new_pools;
            for (i, pool) in &mut self.pools.iter().enumerate() {
                println!(
                    "--- Token Pool {} after processing function: {} ---",
                    i, function.name
                );
                pool.log_contents();
            }
        }

        (self.nodes.clone(), self.edges.clone())
    }
}

#[derive(Serialize)]
struct GraphExport {
    nodes: Vec<Arc<Node>>,
    edges: Vec<Edge>,
}

#[test]
fn main() -> std::io::Result<()> {
    // Define types (constants, variables, errors)
    let initial_command = Arc::new(Variable {
        name: "InitialCommand".to_string(),
        documentation: Some("The initial command input from the CLI".to_string()),
    });
    let path_to_config = Arc::new(Variable {
        name: "PathToConfig".to_string(),
        documentation: Some("The path to the configuration file".to_string()),
    });

    let settings = Arc::new(Constant {
        name: "Settings".to_string(),
        documentation: Some("The loaded settings from the config file".to_string()),
    });

    let templates = Arc::new(Constant {
        name: "Templates".to_string(),
        documentation: Some("The registry of HTML templates used for rendering".to_string()),
    });

    let source_file = Arc::new(Constant {
        name: "SourceFile".to_string(),
        documentation: Some("A raw input file found in the source directory".to_string()),
    });

    let article = Arc::new(Variable {
        name: "Article".to_string(),
        documentation: Some(
            "The processed data structure containing markdown content and metadata".to_string(),
        ),
    });

    let html = Arc::new(Variable {
        name: "HTML".to_string(),
        documentation: Some(
            "The final rendered HTML string ready to be written to disk".to_string(),
        ),
    });

    let fs_error = Arc::new(Error {
        name: "FileSystemError".to_string(),
        documentation: Some(
            "Triggered when a file cannot be read from or written to the disk".to_string(),
        ),
    });

    let success = Arc::new(Variable {
        name: "SuccessReport".to_string(),
        documentation: Some(
            "A final summary of the operations performed during the run".to_string(),
        ),
    });

    // define functions

    let process_cli = Arc::new(Function {
        name: "ProcessCLI".to_string(),
        documentation: Some("Processes command-line input".to_string()),
        consumes: vec![Token::new(
            Arc::new(Kind::Variable(initial_command.clone())),
            Cardinality::Unitary,
            None,
        )],
        produces: vec![
            vec![Token::new(
                Arc::new(Kind::Constant(settings.clone())),
                Cardinality::Unitary,
                None,
            )],
            vec![Token::new(
                Arc::new(Kind::Variable(path_to_config.clone())),
                Cardinality::Unitary,
                None,
            )],
        ],
    });

    let load_config = Arc::new(Function {
        name: "LoadConfig".to_string(),
        documentation: Some("Loads configuration from a file".to_string()),
        consumes: vec![Token::new(
            Arc::new(Kind::Variable(path_to_config.clone())),
            Cardinality::Unitary,
            None,
        )],
        produces: vec![vec![Token::new(
            Arc::new(Kind::Constant(settings.clone())),
            Cardinality::Unitary,
            None,
        )]],
    });
    let load_templates = Arc::new(Function {
        name: "LoadTemplates".to_string(),
        documentation: Some("Loads HTML templates based on settings".to_string()),
        consumes: vec![Token::new(
            Arc::new(Kind::Constant(settings.clone())),
            Cardinality::Unitary,
            None,
        )],
        produces: vec![vec![Token::new(
            Arc::new(Kind::Constant(templates.clone())),
            Cardinality::Unitary,
            None,
        )]],
    });
    let scan_fs = Arc::new(Function {
        name: "ScanFS".to_string(),
        documentation: Some("Scans the filesystem for source files".to_string()),
        consumes: vec![Token::new(
            Arc::new(Kind::Constant(settings.clone())),
            Cardinality::Unitary,
            None,
        )],
        produces: vec![
            vec![Token::new(
                Arc::new(Kind::Constant(source_file.clone())),
                Cardinality::Collection,
                None,
            )],
            vec![Token::new(
                Arc::new(Kind::Error(fs_error.clone())),
                Cardinality::Collection,
                None,
            )],
        ],
    });

    let parse_markdown = Arc::new(Function {
        name: "ParseMarkdown".to_string(),
        documentation: Some("Parses markdown files into article structures".to_string()),
        consumes: vec![Token::new(
            Arc::new(Kind::Constant(source_file.clone())),
            Cardinality::Unitary,
            None,
        )],
        produces: vec![
            vec![Token::new(
                Arc::new(Kind::Variable(article.clone())),
                Cardinality::Unitary,
                None,
            )],
            vec![Token::new(
                Arc::new(Kind::Error(fs_error.clone())),
                Cardinality::Unitary,
                None,
            )],
        ],
    });
    let render_html = Arc::new(Function {
        name: "RenderHTML".to_string(),
        documentation: Some("Renders articles into HTML using templates".to_string()),
        consumes: vec![
            Token::new(
                Arc::new(Kind::Variable(article.clone())),
                Cardinality::Unitary,
                None,
            ),
            Token::new(
                Arc::new(Kind::Constant(templates.clone())),
                Cardinality::Unitary,
                None,
            ),
            Token::new(
                Arc::new(Kind::Constant(settings.clone())),
                Cardinality::Unitary,
                None,
            ),
        ],
        produces: vec![vec![Token::new(
            Arc::new(Kind::Variable(html.clone())),
            Cardinality::Unitary,
            None,
        )]],
    });
    let write_to_disk = Arc::new(Function {
        name: "WriteToDisk".to_string(),
        documentation: Some("Writes HTML files to disk".to_string()),
        consumes: vec![Token::new(
            Arc::new(Kind::Variable(html.clone())),
            Cardinality::Unitary,
            None,
        )],
        produces: vec![
            vec![Token::new(
                Arc::new(Kind::Variable(success.clone())),
                Cardinality::Unitary,
                None,
            )],
            vec![Token::new(
                Arc::new(Kind::Error(fs_error.clone())),
                Cardinality::Unitary,
                None,
            )],
        ],
    });

    let pipeline = vec![
        process_cli,
        load_config,
        load_templates,
        scan_fs,
        parse_markdown,
        render_html,
        write_to_disk,
    ];

    let mut flow = Flow::new();

    let (nodes, edges) = flow.process_flow(&pipeline);

    // Serialization with 4-space indentation
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);

    (GraphExport { nodes, edges }).serialize(&mut ser).unwrap();
    let json_data = String::from_utf8(buf).unwrap();
    let mut file = File::create("../experiments/architecture.json")?;
    file.write_all(json_data.as_bytes())?;
    Ok(())
}
