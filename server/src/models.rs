use serde::Serialize;
use std::fmt;

/// Categorizes architectural entities into discrete roles within the system model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum Kind {
    /// Domain-specific data structures or state containers.
    Data,
    /// Explicit failure states or architectural exceptions.
    Error,
    /// Transformation logic or service contracts.
    Function,
    /// Runtime instances or local architectural state.
    Variable,
    /// Logical modules or namespaces for organizational grouping.
    Group,
    /// Control-flow constructs such as loops or branches.
    Logic,
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Represents an atomic entity in the architectural directed graph.
/// This structure is serialized to JSON for external visualization tools.
#[derive(Debug, Clone, Serialize)]
pub struct Node {
    /// The unique identifier for the node (namespaced, e.g., "def:Credentials").
    pub id: String,
    /// The architectural role of the entity.
    pub kind: Kind,
    /// The human-readable name used for rendering.
    pub label: String,
    /// Optional documentation or metadata associated with the entity.
    pub metadata: Option<String>,
    /// The identifier of the group this node belongs to. Defaults to "global".
    pub group: String,
}

/// Represents a directed relationship between two architectural entities.
#[derive(Debug, Clone, Serialize)]
pub struct Edge {
    /// The ID of the originating architectural node.
    pub source: String,
    /// The ID of the target architectural node.
    pub target: String,
    /// The nature of the connection (e.g., "input_type", "result_flow").
    pub relation: String,
}

/// The root data structure representing the entire extracted architecture.
#[derive(Debug, Default, Serialize)]
pub struct Graph {
    /// A collection of all identified architectural nodes.
    pub nodes: Vec<Node>,
    /// A collection of all identified architectural relationships.
    pub edges: Vec<Edge>,
}

/// Metadata used by the Language Server to provide rich user-facing features.
/// This structure holds the context required for hovers and semantic highlighting.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolInfo {
    /// The architectural category of the symbol.
    pub kind: Kind,
    /// Detailed signature or type information (e.g., "Article -> String").
    pub detail: String,
    /// Cleaned documentation comments formatted for Markdown tooltips.
    pub docs: Option<String>,
    /// The optional group name if the symbol belongs to a logical module.
    pub group: Option<String>,
}
