use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Cardinality {
    #[serde(rename = "1")]
    One,
    #[serde(rename = "*")]
    Many,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Token {
    pub name: String,
    pub is_mutable: bool,
    pub is_collection: bool,
    pub origin_function_uid: Option<u32>,
    pub destination_function_uid: Option<u32>,
}

impl Token {
    pub fn new(name: &str, is_mutable: bool) -> Self {
        Self {
            name: name.to_string(),
            is_mutable,
            is_collection: false,
            origin_function_uid: None,
            destination_function_uid: None,
        }
    }
}

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

    pub fn add(&mut self, mut token: Token, origin_uid: u32, force_collection: bool) {
        token.origin_function_uid = Some(origin_uid);
        if force_collection {
            token.is_collection = true;
        }
        self.available.push(token);
    }

    pub fn consume_requirement(&mut self, req: &Token, consumer_uid: u32) -> (Vec<Token>, bool) {
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
                    t.is_collection = c == Cardinality::Many;
                    t
                })
                .collect(),
            produces: produces
                .into_iter()
                .map(|(mut t, c)| {
                    t.is_collection = c == Cardinality::Many;
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
    let mut engine = FlowProcessor::new();

    let initial_command = Token::new("InitialCommand", true);
    let path_to_config = Token::new("PathToConfig", true);
    let settings = Token::new("Settings", false);
    let templates = Token::new("Templates", false);
    let source_file = Token::new("SourceFile", false);
    let article = Token::new("Article", true);
    let html = Token::new("HTML", true);
    let fs_error = Token::new("FileSystemError", true);
    let success = Token::new("SuccessReport", false);

    let pipeline = vec![
        engine.generate_function(
            "ProcessCLI",
            vec![(initial_command, Cardinality::One)],
            vec![
                (settings.clone(), Cardinality::One),
                (path_to_config.clone(), Cardinality::One),
            ],
            false,
            false,
            false,
        ),
        engine.generate_function(
            "LoadConfig",
            vec![(path_to_config, Cardinality::One)],
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
    let json_data = serde_json::to_string_pretty(&GraphExport { nodes, edges }).unwrap();
    let mut file = File::create("../experiments/architecture.json")?;
    file.write_all(json_data.as_bytes())?;
    Ok(())
}
