//! # Tect Logical Models
//!
//! This module defines the core architectural entities.
//! UIDs are strictly assigned upon construction to ensure logical encapsulation.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

// --- ID Registry ---
static GLOBAL_UID_COUNTER: AtomicU32 = AtomicU32::new(1);

fn next_uid() -> u32 {
    GLOBAL_UID_COUNTER.fetch_add(1, Ordering::SeqCst)
}

// --- Core Logic ---

/// Defines the cardinality of data moving through a contract.
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

/// An immutable data artifact (Constant).
/// In the Token Pool, constants are never consumed; they are
/// available to any function that requires them.
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
/// A mutable data artifact (Variable).
/// Follows linear logic: once consumed by a function, it is
/// removed from the pool unless explicitly re-produced.
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

/// An architectural failure state (Error).
/// Like variables, errors are linear and must be consumed
/// (handled) or they result in a "Fatal" flow termination.

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

// --- Contract Entities ---

/// An instantiation of a Type within a specific function contract.
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

/// A function contract definition.
/// Defines the transformation of input tokens into one or more
/// alternative output pools (branches).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    pub uid: u32,
    pub name: String,
    pub documentation: Option<String>,
    /// The input requirements. Can be empty.
    pub consumes: Vec<Token>,
    /// The output possibilities. Each inner Vec is a separate branch pool.
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

    /// Skeleton creation for discovery pass.
    pub fn new_skeleton(name: String, group: Option<Arc<Group>>) -> Self {
        Self {
            uid: next_uid(),
            name,
            documentation: None,
            consumes: Vec::new(),
            produces: Vec::new(),
            group,
        }
    }
}

// --- Flow Entities (Instances) ---

/// A Node represents a specific execution instance of a function
/// within the 'Flow' section of a Tect file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub uid: u32,
    pub function: Arc<Function>,
    /// Metadata for engine-generated nodes.
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

/// An Edge represents the movement of a specific Token between two Nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub from_node_uid: u32,
    pub to_node_uid: u32,
    pub token: Token,
    /// Semantic label (e.g. "data_flow", "error_branch"). TODO enum?
    pub relation: String,
}

/// The final architectural representation produced by the Engine.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Graph {
    pub nodes: Vec<Arc<Node>>,
    pub edges: Vec<Edge>,
}

// --- LSP Models ---

/// Represents byte offsets in the source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// Used by the LSP to map UIDs back to the source code for
/// features like "Go to Definition" and "Find All References".
#[derive(Debug, Clone)]
pub struct SymbolMetadata {
    pub definition_span: Span,
    pub occurrences: Vec<Span>,
}
