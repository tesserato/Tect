//! # Tect Logical Models
//!
//! This module defines the core architectural entities and the
//! [ProgramStructure] Intermediate Representation (IR).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

// --- ID Registry ---
static GLOBAL_UID_COUNTER: AtomicU32 = AtomicU32::new(1);

fn next_uid() -> u32 {
    GLOBAL_UID_COUNTER.fetch_add(1, Ordering::SeqCst)
}

// --- Core Logic ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd)]
pub enum Cardinality {
    Unitary,
    Collection,
}

// --- Type Definitions (Archetypes) ---

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Group {
    pub uid: u32,
    pub name: String,
    pub documentation: Option<String>,
}

impl Group {
    pub fn new(name: String, documentation: Option<String>) -> Self {
        Self {
            uid: next_uid(),
            name,
            documentation,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Constant {
    pub uid: u32,
    pub name: String,
    pub documentation: Option<String>,
}

impl Constant {
    pub fn new(name: String, documentation: Option<String>) -> Self {
        Self {
            uid: next_uid(),
            name,
            documentation,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Variable {
    pub uid: u32,
    pub name: String,
    pub documentation: Option<String>,
}

impl Variable {
    pub fn new(name: String, documentation: Option<String>) -> Self {
        Self {
            uid: next_uid(),
            name,
            documentation,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Error {
    pub uid: u32,
    pub name: String,
    pub documentation: Option<String>,
}

impl Error {
    pub fn new(name: String, documentation: Option<String>) -> Self {
        Self {
            uid: next_uid(),
            name,
            documentation,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Kind {
    Constant(Arc<Constant>),
    Variable(Arc<Variable>),
    Error(Arc<Error>),
}

impl Kind {
    pub fn uid(&self) -> u32 {
        match self {
            Kind::Constant(c) => c.uid,
            Kind::Variable(v) => v.uid,
            Kind::Error(e) => e.uid,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Kind::Constant(c) => &c.name,
            Kind::Variable(v) => &v.name,
            Kind::Error(e) => &e.name,
        }
    }
}

// --- Contract Entities ---

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Token {
    pub uid: u32,
    pub kind: Kind,
    pub cardinality: Cardinality,
}

impl Token {
    pub fn new(kind: Kind, cardinality: Cardinality) -> Self {
        Self {
            uid: next_uid(),
            kind,
            cardinality,
        }
    }
}

// --- Function Entity ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    pub uid: u32,
    pub name: String,
    pub documentation: Option<String>,
    pub consumes: Vec<Token>,
    pub produces: Vec<Vec<Token>>,
    pub group: Option<Arc<Group>>,
}

impl Function {
    pub fn new(
        name: String,
        documentation: Option<String>,
        consumes: Vec<Token>,
        produces: Vec<Vec<Token>>,
        group: Option<Arc<Group>>,
    ) -> Self {
        Self {
            uid: next_uid(),
            name,
            documentation,
            consumes,
            produces,
            group,
        }
    }

    pub fn new_skeleton(
        name: String,
        documentation: Option<String>,
        group: Option<Arc<Group>>,
    ) -> Self {
        Self {
            uid: next_uid(),
            name,
            documentation,
            consumes: Vec::new(),
            produces: Vec::new(),
            group,
        }
    }
}

// --- Intermediate Representation ---

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProgramStructure {
    pub artifacts: HashMap<String, Kind>,
    pub groups: HashMap<String, Arc<Group>>,
    pub catalog: HashMap<String, Arc<Function>>,
    pub flow: Vec<String>,
    pub symbol_table: HashMap<u32, SymbolMetadata>,
}

// --- Flow Entities ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub uid: u32,
    pub function: Arc<Function>,
    pub is_artificial_graph_start: bool,
    pub is_artificial_graph_end: bool,
    pub is_artificial_error_termination: bool,
}

impl Node {
    pub fn new(function: Arc<Function>) -> Self {
        Self {
            uid: next_uid(),
            function,
            is_artificial_graph_start: false,
            is_artificial_graph_end: false,
            is_artificial_error_termination: false,
        }
    }

    pub fn new_artificial(name: String, is_start: bool, is_end: bool, is_error: bool) -> Self {
        let func = Arc::new(Function::new(
            name,
            Some("Engine-generated boundary node".to_string()),
            vec![],
            vec![],
            None,
        ));
        Self {
            uid: next_uid(),
            function: func,
            is_artificial_graph_start: is_start,
            is_artificial_graph_end: is_end,
            is_artificial_error_termination: is_error,
        }
    }
}

// --- Graph Entities ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub from_node_uid: u32,
    pub to_node_uid: u32,
    pub token: Token,
    /// Semantic label (e.g. "data_flow", "error_branch"). TODO enum?
    pub relation: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

// --- LSP Models ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolMetadata {
    pub definition_span: Span,
    pub occurrences: Vec<Span>,
}
