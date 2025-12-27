use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;

/// Represents the cardinality of data processing.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum Cardinality {
    /// Function processes one item at a time.
    One,
    /// Function processes an entire collection.
    Many,
}

/// A Token represents the state and metadata of data flowing through the pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub name: String,
    pub is_mutable: bool,
    pub is_collection: bool,
    pub origin_uid: Option<u32>,
    pub destination_uid: Option<u32>,
}

impl Token {
    /// Creates a new token based on a name and processing requirements.
    pub fn new(name: &str, is_mutable: bool, is_collection: bool) -> Self {
        Self {
            name: name.to_string(),
            is_mutable,
            is_collection,
            origin_uid: None,
            destination_uid: None,
        }
    }
}

/// A Function represents a discrete processing node in the architecture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
    pub uid: u32,
    pub consumes: Vec<Token>,
    pub produces: Vec<Token>,
    pub is_start: bool,
    pub is_end: bool,
    pub is_error: bool,
}

/// The TokenPool simulates the environment's state, managing resource availability.
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

        // Find the first matching token by name
        if let Some(pos) = self.available.iter().position(|t| t.name == req.name) {
            let mut match_token = self.available[pos].clone();

            // Logic: If input is a collection but consumer expects One -> Trigger Fan-out
            if match_token.is_collection && !req.is_collection {
                triggered_expansion = true;
            }

            let mut edge = match_token.clone();
            edge.destination_uid = Some(consumer_uid);
            edges.push(edge);

            self.consumed.push(match_token.clone());

            // If the resource is mutable, it is removed from the environment
            if match_token.is_mutable {
                self.available.remove(pos);
            }
        }

        (edges, triggered_expansion)
    }
}

/// Orchestrates the flow of functions, managing UID generation and terminal node routing.
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

    /// Processes a list of logical functions and generates the full architectural graph.
    pub fn process_flow(&mut self, pipeline: Vec<Function>) -> (Vec<Function>, Vec<Token>) {
        let mut pool = TokenPool::new();
        let mut all_nodes = Vec::new();
        let mut all_edges = Vec::new();

        let start_node = self.create_node("Start", vec![], vec![], true, false, false);
        let end_node = self.create_node("End", vec![], vec![], false, true, false);

        // Seed initial tokens from the Start node
        if let Some(first) = pipeline.first() {
            for req in &first.consumes {
                pool.add(req.clone(), start_node.uid, false);
            }
        }
        all_nodes.push(start_node);

        // Process logic nodes
        for mut func in pipeline {
            let mut func_is_expanded = false;

            // 1. Resolve requirements
            for req in &func.consumes {
                let (edges, expanded) = pool.consume_requirement(req, func.uid);
                all_edges.extend(edges);
                if expanded {
                    func_is_expanded = true;
                }
            }

            // 2. Add products to pool (propagating expansion status)
            let products = func.produces.clone();
            for prod in products {
                pool.add(prod, func.uid, func_is_expanded);
            }

            all_nodes.push(func);
        }

        // Route leftovers to End or Error nodes
        let mut error_nodes = std::collections::HashMap::new();

        // We iterate and move leftovers out of the pool
        let leftovers: Vec<Token> = pool.available.drain(..).collect();
        for mut leftover in leftovers {
            let target_uid = if leftover.name.contains("Error") {
                let name = leftover.name.clone();
                let uid = *error_nodes.entry(name.clone()).or_insert_with(|| {
                    let err_node = self.create_node(&name, vec![], vec![], false, true, true);
                    let id = err_node.uid;
                    all_nodes.push(err_node);
                    id
                });
                uid
            } else {
                end_node.uid
            };

            leftover.destination_uid = Some(target_uid);
            all_edges.push(leftover);
        }

        all_nodes.push(end_node);
        (all_nodes, all_edges)
    }

    /// Helper to generate a function node with a unique ID.
    pub fn create_node(
        &mut self,
        name: &str,
        consumes: Vec<(Token, Cardinality)>,
        produces: Vec<(Token, Cardinality)>,
        is_start: bool,
        is_end: bool,
        is_error: bool,
    ) -> Function {
        let mut c_tokens = Vec::new();
        for (mut t, card) in consumes {
            t.is_collection = card == Cardinality::Many;
            c_tokens.push(t);
        }

        let mut p_tokens = Vec::new();
        for (mut t, card) in produces {
            t.is_collection = card == Cardinality::Many;
            p_tokens.push(t);
        }

        Function {
            name: name.to_string(),
            uid: self.next_uid(),
            consumes: c_tokens,
            produces: p_tokens,
            is_start,
            is_end,
            is_error,
        }
    }
}

/// Container for the serialized output.
#[derive(Serialize)]
struct GraphExport {
    nodes: Vec<Function>,
    edges: Vec<Token>,
}

#[test]
fn main() -> std::io::Result<()> {
    let mut engine = FlowProcessor::new();

    // Define Domain Tokens
    let cmd = Token::new("InitialCommand", true, false);
    let settings = Token::new("Settings", false, false);
    let source_file = Token::new("SourceFile", false, false);
    let fs_error = Token::new("FileSystemError", true, false);

    // Build Pipeline
    let pipeline = vec![
        engine.create_node(
            "ProcessCLI",
            vec![(cmd, Cardinality::One)],
            vec![(settings.clone(), Cardinality::One)],
            false,
            false,
            false,
        ),
        engine.create_node(
            "ScanFS",
            vec![(settings, Cardinality::One)],
            vec![
                (source_file, Cardinality::Many),
                (fs_error, Cardinality::Many),
            ],
            false,
            false,
            false,
        ),
    ];

    // Execute Logic
    let (nodes, edges) = engine.process_flow(pipeline);

    // Save to JSON
    let export = GraphExport { nodes, edges };
    let json_data = serde_json::to_string_pretty(&export).unwrap();

    let mut file = File::create("architecture.json")?;
    file.write_all(json_data.as_bytes())?;

    println!("Rust architecture processing complete. JSON saved.");
    Ok(())
}
