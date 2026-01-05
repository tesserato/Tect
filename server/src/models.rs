//! # Tect Logical Models
//!
//! This module defines the core architectural entities.
//! UIDs are the source of truth for identity. Spans are handled by the Analyzer's SymbolTable.

use serde::{Deserialize, Serialize};
use std::sync::Arc;

// --- ID Registry ---
macro_rules! define_id_registry {
    ($name:ident) => {
        mod $name {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(1);
            pub fn next() -> u32 {
                COUNTER.fetch_add(1, Ordering::SeqCst)
            }
        }
    };
}
define_id_registry!(token_id_registry);
define_id_registry!(func_id_registry);
define_id_registry!(type_id_registry);
define_id_registry!(node_id_registry);

// --- Core Logic ---

/// Defines the cardinality of data moving through a contract.
/// Used by the Engine to determine if a transformation represents
/// a single operation or an iterative/collection operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd)]
pub enum Cardinality {
    Unitary,
    Collection,
}

// --- Type Definitions (Archetypes) ---

/// A logical architectural container (Cluster).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Group {
    pub uid: u32,
    pub name: String,
    pub documentation: Option<String>,
}

impl Group {
    pub fn new(name: String, docs: Option<String>) -> Self {
        Self {
            uid: type_id_registry::next(),
            name,
            documentation: docs,
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

/// A mutable data artifact (Variable).
/// Follows linear logic: once consumed by a function, it is
/// removed from the pool unless explicitly re-produced.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Variable {
    pub uid: u32,
    pub name: String,
    pub documentation: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "kind", content = "data")]
pub enum Kind {
    Constant(Arc<Constant>),
    Variable(Arc<Variable>),
    Error(Arc<Error>),
}

// --- Contract Entities ---

/// An instantiation of a Type within a specific function contract.
/// A single `Kind` (e.g. `Article`) might be used as a `Token`
/// in many different functions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Token {
    pub uid: u32,
    pub kind: Kind,
    pub cardinality: Cardinality,
}

impl Token {
    pub fn new(kind: Kind, cardinality: Cardinality) -> Self {
        Self {
            uid: token_id_registry::next(),
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
    /// Optional logical group ownership.
    pub group: Option<Arc<Group>>,
}

impl Function {
    pub fn new(name: String, docs: Option<String>, group: Option<Arc<Group>>) -> Self {
        Self {
            uid: func_id_registry::next(),
            name,
            documentation: docs,
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
            uid: node_id_registry::next(),
            function,
            is_artificial_graph_start: false,
            is_artificial_graph_end: false,
            is_artificial_error_termination: false,
        }
    }
}

// --- Graph Entities ---

/// An Edge represents the movement of a specific Token between two Nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// UID of the node producing the token.
    pub from_node_uid: u32,
    /// UID of the node consuming the token.
    pub to_node_uid: u32,
    /// The specific token instance being passed.
    pub token: Token,
    /// Semantic label (e.g. "data_flow", "error_branch").
    pub relation: String,
}

/// The final architectural representation produced by the Engine.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Graph {
    pub nodes: Vec<Node>,
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
