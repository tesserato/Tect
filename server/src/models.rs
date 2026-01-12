//! # Tect Logical Models
//!
//! Defines the core architectural entities, the Intermediate Representation (IR),
//! and the diagnostic structures used across the compiler pipeline.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tower_lsp::lsp_types::{DiagnosticSeverity, DiagnosticTag};

// --- Hashing Helper ---

/// Computes a deterministic hash for a given name string.
///
/// This function uses a custom FNV-1a implementation to ensure cross-platform
/// and cross-run determinism, which is essential for consistent UIDs in
/// serialized outputs (like JSON or stable graph generation).
pub fn hash_name(name: &str) -> u32 {
    let mut hash: u32 = 0x811c9dc5;
    for byte in name.bytes() {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    hash
}

// --- Source Management Types ---

/// A unique identifier for a source file within the `SourceManager`.
pub type FileId = u32;

/// Represents a span of text in a source file.
///
/// A span corresponds to a contiguous range of bytes within a specific file.
/// It is used for error reporting and mapping IR elements back to the source code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    /// The ID of the file containing this span.
    pub file_id: FileId,
    /// The simplified 0-indexed byte offset of the start of the span.
    pub start: usize,
    /// The simplified 0-indexed byte offset of the end of the span.
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

/// Represents a compiler diagnostic message including context information.
///
/// This structure holds the information required to report an error or warning
/// to the user, potentially including a specific source span.
#[derive(Debug, Clone)]
pub struct DiagnosticWithContext {
    /// ID of the file associated with the diagnostic.
    pub file_id: FileId,
    /// The specific span within the file, if applicable.
    pub span: Option<Span>,
    /// The diagnostic message intended for the user.
    pub message: String,
    /// The severity level (Error, Warning, Information, Hint).
    pub severity: DiagnosticSeverity,
    /// Additional tags (e.g., Unnecessary, Deprecated).
    pub tags: Vec<DiagnosticTag>,
}

// --- Core Logic ---

/// Defines the cardinality of a token or data element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd)]
pub enum Cardinality {
    /// Represents a single item.
    Unitary,
    /// Represents a collection of items (e.g., list, set).
    Collection,
}

/// Semantic relationship type for edges in the architectural graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeRelation {
    /// Resembles value propagation.
    DataFlow,
    /// Represents terminal signal flow.
    TerminalFlow,
    /// Represents error propagation flow.
    ErrorFlow,
    /// Represents logical control flow (branching, sequencing).
    ControlFlow,
    /// Represents a direct function call relationship.
    Call,
}

// --- Type Definitions (Archetypes) ---

/// Represents a logical grouping of functions or components.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Group {
    /// Unique identifier for the group (hash of name).
    pub uid: u32,
    /// The user-facing name of the group.
    pub name: String,
    /// Optional documentation string.
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

/// Represents a constant value definition in the IR.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Constant {
    /// Unique identifier for the constant (hash of name).
    pub uid: u32,
    /// The name of the constant.
    pub name: String,
    /// Optional documentation.
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

/// Represents a variable definition in the IR.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Variable {
    /// Unique identifier for the variable (hash of name).
    pub uid: u32,
    /// The name of the variable.
    pub name: String,
    /// Optional documentation.
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

/// Represents an error type definition in the IR.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Error {
    /// Unique identifier for the error (hash of name).
    pub uid: u32,
    /// The name of the error.
    pub name: String,
    /// Optional documentation.
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
    /// Returns the unique identifier of the inner kind.
    pub fn uid(&self) -> u32 {
        match self {
            Kind::Constant(c) => c.uid,
            Kind::Variable(v) => v.uid,
            Kind::Error(e) => e.uid,
        }
    }

    /// Returns the name of the inner kind.
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

/// Represents a token instance utilized in a function signature or flow.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Token {
    /// Unique identifier from the associated type (Kind).
    pub uid: u32,
    /// The kind of the token (Constant, Variable, or Error).
    pub kind: Kind,
    /// The cardinality of the token usage (Unitary or Collection).
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

/// Represents a function definition in the IR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    /// Unique identifier for the function (hash of name).
    pub uid: u32,
    /// The function's name.
    pub name: String,
    /// Optional documentation.
    pub documentation: Option<String>,
    /// List of tokens consumed (input arguments).
    pub consumes: Vec<Token>,
    /// List of tokens produced (return values), supporting multiple output paths.
    pub produces: Vec<Vec<Token>>,
    /// The logical group this function belongs to (if any).
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

/// Represents the complete structure of a parsed Tect program.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProgramStructure {
    /// Map of artifact names to their definitions (Kind).
    pub artifacts: HashMap<String, Kind>,
    /// Map of group names to Group definitions.
    pub groups: HashMap<String, Arc<Group>>,
    /// Map of function names to Function definitions.
    pub catalog: HashMap<String, Arc<Function>>,
    /// The ordered sequence of flow steps defined in the program.
    pub flow: Vec<FlowStep>,
    /// Symbol table for looking up definition spans and occurrences.
    pub symbol_table: HashMap<u32, SymbolMetadata>,
    /// Diagnostics collected during parsing and analysis.
    #[serde(skip)]
    pub diagnostics: Vec<DiagnosticWithContext>,
}

/// Represents a step in the execution flow.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FlowStep {
    /// The name of the function invoked at this step.
    pub function_name: String,
    /// The source span where this step is defined.
    pub span: Span,
}

// --- Flow Entities ---

/// Represents a node in the execution graph suitable for visualization or analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// Unique identifier for the node.
    pub uid: u32,
    /// The function associated with this node.
    pub function: Arc<Function>,
    /// Indicates if this is an artificial start node.
    pub is_artificial_graph_start: bool,
    /// Indicates if this is an artificial end node.
    pub is_artificial_graph_end: bool,
    /// Indicates if this is an artificial error termination node.
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
    /// Creates a new graph node from a function definition.
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

/// Represents a directed edge in the execution graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Edge {
    /// UID of the source node.
    pub from_node_uid: u32,
    /// UID of the destination node.
    pub to_node_uid: u32,
    /// The token being carried along this edge.
    pub token: Token,
    /// The relationship type of this edge.
    pub relation: EdgeRelation,
}

/// Represents the full execution graph.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Graph {
    /// The collection of nodes in the graph.
    pub nodes: Vec<Node>,
    /// The collection of edges connecting the nodes.
    pub edges: Vec<Edge>,
}

// --- Symbol Metadata ---

/// Metadata supporting symbol lookup and "go to definition".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolMetadata {
    /// The name of the symbol.
    pub name: String,
    /// The span where the symbol is defined.
    pub definition_span: Span,
    /// List of spans wherever the symbol is referenced.
    pub occurrences: Vec<Span>,
}
