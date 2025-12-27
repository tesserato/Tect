use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;

/// Defines whether a function handles a single item or a collection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cardinality {
    One,
    Many,
}

/// Represents data moving through the system.
/// Renamed fields via serde to match Python's output exactly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub name: String,
    pub is_mutable: bool,
    pub is_collection: bool,
    #[serde(rename = "origin_function_uid")]
    pub origin_uid: Option<u32>,
    #[serde(rename = "destination_function_uid")]
    pub destination_uid: Option<u32>,
}

impl Token {
    pub fn new(name: &str, is_mutable: bool) -> Self {
        Self {
            name: name.to_string(),
            is_mutable,
            is_collection: false,
            origin_uid: None,
            destination_uid: None,
        }
    }
}

/// The 'Node' in the graph.
/// Uses exact field names from the Python version for JSON compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
    pub uid: u32,
    pub consumes: Vec<Token>,
    pub produces: Vec<Token>,
    pub is_artificial_graph_start: bool,
    pub is_artificial_graph_end: bool,
    pub is_artificial_error_termination: bool,
}

/// Manages available tokens and handles logic for mutable vs. immutable consumption.
pub struct TokenPool {
    pub available: Vec<Token>,
    pub consumed: Vec<Token>,
}

impl TokenPool {
    pub fn new() -> Self {
        Self {
            available: Vec::new(),
            consumed: Vec::new(),
        }
    }

    /// Adds a produced token to the pool.
    /// If `force_collection` is true, the token is treated as a collection (Fan-out propagation).
    pub fn add(&mut self, mut token: Token, origin_uid: u32, force_collection: bool) {
        token.origin_uid = Some(origin_uid);
        if force_collection {
            token.is_collection = true;
        }
        self.available.push(token);
    }

    /// Attempts to satisfy a function requirement.
    /// Returns a tuple containing the successful edges and a boolean indicating if a Fan-out occurred.
    pub fn consume_requirement(&mut self, req: &Token, consumer_uid: u32) -> (Vec<Token>, bool) {
        let mut edges = Vec::new();
        let mut triggered_expansion = false;

        // Find match in pool
        if let Some(pos) = self.available.iter().position(|t| t.name == req.name) {
            let mut matched = self.available[pos].clone();

            // Fan-out Logic: Input is collection, but function consumes ONE
            if matched.is_collection && !req.is_collection {
                triggered_expansion = true;
            }

            let mut edge = matched.clone();
            edge.destination_uid = Some(consumer_uid);
            edges.push(edge);

            self.consumed.push(matched.clone());
            if matched.is_mutable {
                self.available.remove(pos);
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

    fn next_uid(&mut self) -> u32 {
        let id = self.uid_counter;
        self.uid_counter += 1;
        id
    }

    /// Factory to generate a function node.
    pub fn generate_function(
        &mut self,
        name: &str,
        consumes: Vec<(Token, Cardinality)>,
        produces: Vec<(Token, Cardinality)>,
        is_start: bool,
        is_end: bool,
        is_error: bool,
    ) -> Function {
        let c_tokens = consumes
            .into_iter()
            .map(|(mut t, c)| {
                t.is_collection = c == Cardinality::Many;
                t
            })
            .collect();

        let p_tokens = produces
            .into_iter()
            .map(|(mut t, c)| {
                t.is_collection = c == Cardinality::Many;
                t
            })
            .collect();

        Function {
            name: name.to_string(),
            uid: self.next_uid(),
            consumes: c_tokens,
            produces: p_tokens,
            is_artificial_graph_start: is_start,
            is_artificial_graph_end: is_end,
            is_artificial_error_termination: is_error,
        }
    }

    /// Processes the flow and handles terminal/error routing.
    pub fn process_flow(&mut self, pipeline: Vec<Function>) -> (Vec<Function>, Vec<Token>) {
        let mut pool = TokenPool::new();
        let mut nodes = Vec::new();
        let mut all_edges = Vec::new();

        let start_node = self.generate_function("Start", vec![], vec![], true, false, false);
        let mut end_node = self.generate_function("End", vec![], vec![], false, true, false);

        // Seed pool from first node's requirements via Start node
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

        // Terminal Routing
        let mut error_nodes: HashMap<String, Function> = HashMap::new();
        let leftovers: Vec<Token> = pool.available.drain(..).collect();

        for mut leftover in leftovers {
            let target_uid = if leftover.name.contains("Error") {
                let name = leftover.name.clone();
                let err_node = error_nodes.entry(name.clone()).or_insert_with(|| {
                    self.generate_function(&name, vec![], vec![], false, true, true)
                });
                err_node.uid
            } else {
                end_node.uid
            };

            leftover.destination_uid = Some(target_uid);
            all_edges.push(leftover);
        }

        // Collect generated error nodes and finally the End node
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
    let mut engine = FlowProcessor::new();

    // --- Domain Tokens ---
    let initial_cmd = Token::new("InitialCommand", true);
    let path_to_cfg = Token::new("PathToConfig", true);
    let settings = Token::new("Settings", false); // Immutable
    let templates = Token::new("Templates", false); // Immutable (MISSING TYPE ADDED)
    let source_file = Token::new("SourceFile", false); // Immutable
    let article = Token::new("Article", true);
    let html = Token::new("HTML", true);
    let fs_error = Token::new("FileSystemError", true);
    let success = Token::new("SuccessReport", false);

    // --- Complete Pipeline (Mirroring Python logic exactly) ---
    let pipeline = vec![
        engine.generate_function(
            "ProcessCLI",
            vec![(initial_cmd, Cardinality::One)],
            vec![
                (settings.clone(), Cardinality::One),
                (path_to_cfg.clone(), Cardinality::One),
            ],
            false,
            false,
            false,
        ),
        engine.generate_function(
            "LoadConfig",
            vec![(path_to_cfg, Cardinality::One)],
            vec![(settings.clone(), Cardinality::One)],
            false,
            false,
            false,
        ),
        engine.generate_function(
            "LoadTemplates",
            vec![(settings.clone(), Cardinality::One)],
            vec![(templates.clone(), Cardinality::One)],
            false,
            false,
            false,
        ),
        engine.generate_function(
            "ScanFS",
            vec![(settings.clone(), Cardinality::One)],
            vec![
                (source_file.clone(), Cardinality::Many),
                (fs_error.clone(), Cardinality::Many),
            ],
            false,
            false,
            false,
        ),
        engine.generate_function(
            "ParseMarkdown",
            vec![(source_file, Cardinality::One)],
            vec![
                (article.clone(), Cardinality::One),
                (fs_error.clone(), Cardinality::One),
            ],
            false,
            false,
            false,
        ),
        engine.generate_function(
            "RenderHTML",
            vec![
                (article, Cardinality::One),
                (templates, Cardinality::One),
                (settings, Cardinality::One),
            ],
            vec![(html.clone(), Cardinality::One)],
            false,
            false,
            false,
        ),
        engine.generate_function(
            "WriteToDisk",
            vec![(html, Cardinality::Many)],
            vec![(success, Cardinality::One), (fs_error, Cardinality::Many)],
            false,
            false,
            false,
        ),
    ];

    let (nodes, edges) = engine.process_flow(pipeline);

    // Save to JSON
    let export = GraphExport { nodes, edges };
    let json_data = serde_json::to_string_pretty(&export).unwrap();

    let mut file = File::create("../experiments/architecture.json")?;
    file.write_all(json_data.as_bytes())?;

    println!("Success: architecture.json generated with exact field mapping.");
    Ok(())
}
