use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Node {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub metadata: Option<String>,
    pub group: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Edge {
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
    pub kind: String,
    pub detail: String,
    pub docs: Option<String>,
    pub group: Option<String>,
}
