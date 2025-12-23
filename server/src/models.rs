use serde::Serialize;

/// Represents a single node in the architectural directed graph.
/// Nodes represent Data entities, Errors, Functions, Variables, or logic blocks.
#[derive(Debug, Clone, Serialize)]
pub struct Node {
    /// Unique identifier for the node (e.g., "def:Settings" or "var:appSettings").
    pub id: String,
    /// The architectural category of the node (Data, Error, Function, etc.).
    pub kind: String,
    /// The display name of the node in the generated graph.
    pub label: String,
    /// Extracted documentation comments associated with the definition.
    pub metadata: Option<String>,
    /// The logical group (module) this node belongs to. Defaults to "global".
    pub group: String,
}

/// Represents a directed relationship between two architectural nodes.
#[derive(Debug, Clone, Serialize)]
pub struct Edge {
    /// The source node ID where the relationship begins.
    pub source: String,
    /// The target node ID where the relationship ends.
    pub target: String,
    /// The type of relationship (e.g., "argument_flow", "type_definition").
    pub relation: String,
}

/// The root container for the architectural graph, designed for language-agnostic JSON export.
#[derive(Debug, Default, Serialize)]
pub struct Graph {
    /// Collection of architectural entities.
    pub nodes: Vec<Node>,
    /// Collection of architectural relationships.
    pub edges: Vec<Edge>,
}

/// Metadata used internally by the LSP to provide rich tooltips and semantic analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolInfo {
    /// The primary role of the symbol (Group, Data, Function, etc.).
    pub kind: String,
    /// Detailed type information (e.g., "InputType -> OutputType").
    pub detail: String,
    /// Cleaned documentation strings for Markdown rendering.
    pub docs: Option<String>,
    /// The logical group context of the symbol.
    pub group: Option<String>,
}
