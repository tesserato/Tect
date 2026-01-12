//! # Tect Logical Models
//!
//! Defines the core architectural entities, the Intermediate Representation (IR),
//! and the diagnostic structures used across the compiler pipeline.

use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tower_lsp::lsp_types::{DiagnosticSeverity, DiagnosticTag};

// --- Hashing Helper ---

pub fn hash_name(name: &str) -> u32 {
    let mut s = DefaultHasher::new();
    name.hash(&mut s);
    (s.finish() & 0xFFFFFFFF) as u32
}

// --- Source Management Types ---

pub type FileId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub file_id: FileId,
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(file_id: FileId, start: usize, end: usize) -> Self {
        Self {
            file_id,
            start,
            end,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiagnosticWithContext {
    pub file_id: FileId,
    pub span: Option<Span>,
    pub message: String,
    pub severity: DiagnosticSeverity,
    pub tags: Vec<DiagnosticTag>,
}

// --- Core Logic ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd)]
pub enum Cardinality {
    Unitary,
    Collection,
}

/// Semantic relationship type for edges in the architectural graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeRelation {
    DataFlow,
    TerminalFlow,
    ErrorFlow,
    ControlFlow,
    Call,
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
            uid: hash_name(&name),
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
            uid: hash_name(&name),
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
            uid: hash_name(&name),
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
            uid: hash_name(&name),
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

    pub fn docs(&self) -> Option<&str> {
        match self {
            Kind::Constant(c) => c.documentation.as_deref(),
            Kind::Variable(v) => v.documentation.as_deref(),
            Kind::Error(e) => e.documentation.as_deref(),
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
    /// Creates a token with a deterministic UID based on context.
    pub fn new(kind: Kind, cardinality: Cardinality, uid: u32) -> Self {
        Self {
            uid,
            kind,
            cardinality,
        }
    }
}

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
            uid: hash_name(&name),
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
            uid: hash_name(&name),
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
    pub flow: Vec<FlowStep>,
    pub symbol_table: HashMap<u32, SymbolMetadata>,
    #[serde(skip)]
    pub diagnostics: Vec<DiagnosticWithContext>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FlowStep {
    pub function_name: String,
    pub span: Span,
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

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.uid == other.uid
            && self.is_artificial_graph_start == other.is_artificial_graph_start
            && self.is_artificial_graph_end == other.is_artificial_graph_end
            && self.is_artificial_error_termination == other.is_artificial_error_termination
    }
}
impl Eq for Node {}

// Manual Hash to match PartialEq and avoid hashing Arc<Function> content redundantly
impl Hash for Node {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.uid.hash(state);
        self.is_artificial_graph_start.hash(state);
        self.is_artificial_graph_end.hash(state);
        self.is_artificial_error_termination.hash(state);
    }
}

impl Node {
    pub fn new(function: Arc<Function>) -> Self {
        Self {
            uid: function.uid,
            function,
            is_artificial_graph_start: false,
            is_artificial_graph_end: false,
            is_artificial_error_termination: false,
        }
    }

    pub fn new_artificial(name: String, is_start: bool, is_end: bool, is_error: bool) -> Self {
        let uid = hash_name(&name);
        let func = Arc::new(Function::new(
            name,
            Some("Engine-generated boundary node".to_string()),
            vec![],
            vec![],
            None,
        ));
        Self {
            uid,
            function: func,
            is_artificial_graph_start: is_start,
            is_artificial_graph_end: is_end,
            is_artificial_error_termination: is_error,
        }
    }
}

// --- Graph Entities ---

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Edge {
    pub from_node_uid: u32,
    pub to_node_uid: u32,
    pub token: Token,
    pub relation: EdgeRelation,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

// --- Symbol Metadata ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolMetadata {
    pub name: String,
    pub definition_span: Span,
    pub occurrences: Vec<Span>,
}
