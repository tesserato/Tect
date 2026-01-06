//! # Tect Logical Models
//!
//! This module defines the core architectural entities and the
//! [ProgramStructure] Intermediate Representation (IR).
//!
//! UIDs are strictly encapsulated and assigned automatically upon
//! construction to ensure global uniqueness within the process.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

// --- ID Registry ---
static GLOBAL_UID_COUNTER: AtomicU32 = AtomicU32::new(1);

/// Internal helper to fetch the next unique identifier.
fn next_uid() -> u32 {
    GLOBAL_UID_COUNTER.fetch_add(1, Ordering::SeqCst)
}

// --- Core Logic ---

/// Defines the cardinality of data moving through a contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd)]
pub enum Cardinality {
    /// A single instance of an artifact.
    Unitary,
    /// A set or stream of artifact instances.
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
    /// Creates a new logical group with an encapsulated UID.
    pub fn new(name: String, documentation: Option<String>) -> Self {
        Self {
            uid: next_uid(),
            name,
            documentation,
        }
    }
}

/// An immutable data artifact (Constant).
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

/// Sum type of all architectural data artifacts.
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
    /// Creates a new token instance with an encapsulated UID.
    pub fn new(kind: Kind, cardinality: Cardinality) -> Self {
        Self {
            uid: next_uid(),
            kind,
            cardinality,
        }
    }
}

/// A function contract definition (Transformation).
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
    /// Creates a complete function contract.
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

    /// Creates a function skeleton during parsing passes.
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

// --- Intermediate Representation ---

/// The decoupled Intermediate Representation (IR) of a Tect architecture.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProgramStructure {
    /// Mapping of artifact names to their semantic kinds.
    pub artifacts: HashMap<String, Kind>,
    /// Mapping of group names to their logical definitions.
    pub groups: HashMap<String, Arc<Group>>,
    /// Mapping of function names to their contract definitions.
    pub catalog: HashMap<String, Arc<Function>>,
    /// The ordered sequence of function calls to be executed in the flow.
    pub flow: Vec<String>,
}

// --- Flow Entities (Instances) ---

/// A Node represents a specific execution instance of a function.
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
    /// Instantiates a node from a function definition.
    pub fn new(function: Arc<Function>) -> Self {
        Self {
            uid: next_uid(),
            function,
            is_artificial_graph_start: false,
            is_artificial_graph_end: false,
            is_artificial_error_termination: false,
        }
    }

    /// Creates a boundary node for graph entry/exit logic.
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

/// Represents the movement of data between two nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub from_node_uid: u32,
    pub to_node_uid: u32,
    pub token: Token,
    /// Semantic label (e.g. "data_flow", "error_branch"). TODO enum?
    pub relation: String,
}

/// The final visual/logical graph output.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

// --- LSP Models ---

/// Byte offsets in source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// Metadata mapping logical entities to source locations.
#[derive(Debug, Clone)]
pub struct SymbolMetadata {
    pub definition_span: Span,
    pub occurrences: Vec<Span>,
}
