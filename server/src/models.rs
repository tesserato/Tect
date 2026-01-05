use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;

// ==========================================================
// 1. UID ENCAPSULATION SETUP
// ==========================================================

/// This macro creates a private module containing a hidden atomic counter.
/// It provides a clean way to reuse UID logic across different types.
macro_rules! define_id_generator {
    ($name:ident) => {
        mod $name {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            pub fn next() -> u32 {
                COUNTER.fetch_add(1, Ordering::SeqCst)
            }
        }
    };
}

// Generate independent registries for each type that needs UIDs
define_id_generator!(token_id_registry);
define_id_generator!(node_id_registry);

// ==========================================================
// 2. DATA MODELS
// ==========================================================

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Eq, Hash, PartialOrd)]
pub enum Cardinality {
    Unitary,
    Collection,
}

#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq, PartialOrd)]
pub struct Group {
    pub name: String,
    pub documentation: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq, PartialOrd)]
pub struct Variable {
    pub name: String,
    pub documentation: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq, PartialOrd)]
pub struct Constant {
    pub name: String,
    pub documentation: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq, PartialOrd)]
pub struct Error {
    pub name: String,
    pub documentation: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq, PartialOrd)]
pub enum Kind {
    Variable(Arc<Variable>),
    Constant(Arc<Constant>),
    Error(Arc<Error>),
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::Variable(v) => write!(f, "Variable ({})", v.name),
            Kind::Constant(c) => write!(f, "Constant ({})", c.name),
            Kind::Error(e) => write!(f, "Error ({})", e.name),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq, PartialOrd)]
pub struct Function {
    pub name: String,
    pub documentation: Option<String>,
    pub consumes: Vec<Token>,
    pub produces: Vec<Vec<Token>>,
    pub group: Option<Arc<Group>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq, PartialOrd)]
pub struct Token {
    pub uid: u32,
    pub kind: Arc<Kind>,
    pub cardinality: Cardinality,
}

impl Token {
    pub fn new(kind: Arc<Kind>, cardinality: Cardinality) -> Self {
        Self {
            uid: token_id_registry::next(), // Logic encapsulated
            kind,
            cardinality,
        }
    }

    // pub fn compare(&self, other: &Self) -> bool {
    //     self.kind == other.kind
    // }
}


// Represents a node in the semantic graph - roughly corresponds to a function instance
#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq)]
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
            uid: node_id_registry::next(),
            function,
            is_artificial_graph_start: false,
            is_artificial_graph_end: false,
            is_artificial_error_termination: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq, PartialOrd)]
pub struct Edge {
    pub origin_function: Arc<Function>,
    pub destination_function: Arc<Function>,
    pub token: Token,
    pub source: String,
    pub target: String,
    pub relation: String,
}

#[derive(Debug, Default, Serialize)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SymbolInfo {
    pub kind: Kind,
    pub detail: String,
    pub docs: Option<String>,
    pub group: Option<String>,
}
