use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

static TOKEN_ID_COUNTER: AtomicU32 = AtomicU32::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Eq, Hash, PartialOrd)]
pub enum Cardinality {
    Unitary,
    Collection,
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
    // Legacy support for LSP simple analyzer
    Data,
    Function,
    Group,
    Logic,
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::Variable(v) => write!(f, "Variable ({})", v.name),
            Kind::Constant(c) => write!(f, "Constant ({})", c.name),
            Kind::Error(e) => write!(f, "Error ({})", e.name),
            other => write!(f, "{:?}", other),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq, PartialOrd)]
pub struct Group {
    pub name: String,
    pub documentation: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq, PartialOrd)]
pub struct Function {
    pub name: String,
    pub documentation: Option<String>,
    pub consumes: Vec<Token>,
    pub produces: Vec<Vec<Token>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq, PartialOrd)]
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
        self.kind == other.kind
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq)]
pub struct Node {
    pub uid: u32,
    pub function: Arc<Function>,
    pub is_artificial_graph_start: bool,
    pub is_artificial_graph_end: bool,
    pub is_artificial_error_termination: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq, PartialOrd)]
pub struct Edge {
    pub origin_function: Arc<Function>,
    pub destination_function: Arc<Function>,
    pub token: Token,
    // Legacy support for simple graphviz
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
