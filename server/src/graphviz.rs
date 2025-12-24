use crate::models::{Graph, Kind};
use std::collections::HashMap;
use std::fmt::Write;

/// Converts the semantic architectural graph into Graphviz DOT.
///
/// Rendering rules:
/// - `Data` and `Error` definitions are NOT rendered
/// - Variable nodes embed their type directly
/// - Long documentation is exposed via tooltips only
/// - Groups are mapped to clusters
pub fn to_dot(graph: &Graph) -> String {
    let mut out = String::new();

    writeln!(out, "digraph Tect {{").unwrap();
    writeln!(out, "  rankdir=LR;").unwrap();
    writeln!(out, "  compound=true;").unwrap();
    writeln!(out, "  node [shape=box, fontname=\"Inter\"];").unwrap();

    // Group nodes by cluster
    let mut groups: HashMap<&str, Vec<&crate::models::Node>> = HashMap::new();
    for node in &graph.nodes {
        // Skip pure type definitions
        if matches!(node.kind, Kind::Data | Kind::Error) {
            continue;
        }
        groups.entry(&node.group).or_default().push(node);
    }

    for (group, nodes) in groups {
        let clustered = group != "global";

        if clustered {
            writeln!(out, "  subgraph cluster_{} {{", sanitize(group)).unwrap();
            writeln!(out, "    label=\"{}\";", group).unwrap();
            writeln!(out, "    style=rounded;").unwrap();
        }

        for n in nodes {
            let label = node_label(n);
            let tooltip = n.metadata.clone().unwrap_or_default();

            writeln!(
                out,
                "    \"{}\" [label=<{}>, tooltip=\"{}\", class=\"{:?}\"];",
                n.id,
                label,
                escape(&tooltip),
                n.kind
            )
            .unwrap();
        }

        if clustered {
            writeln!(out, "  }}").unwrap();
        }
    }

    // Render edges (excluding edges pointing to hidden nodes)
    for e in &graph.edges {
        if graph.nodes.iter().any(|n| {
            n.id == e.source && !matches!(n.kind, Kind::Data | Kind::Error)
        }) && graph.nodes.iter().any(|n| {
            n.id == e.target && !matches!(n.kind, Kind::Data | Kind::Error)
        }) {
            writeln!(
                out,
                "  \"{}\" -> \"{}\" [label=\"{}\"];",
                e.source, e.target, e.relation
            )
            .unwrap();
        }
    }

    writeln!(out, "}}").unwrap();
    out
}

/// Generates a clean, compact node label.
fn node_label(n: &crate::models::Node) -> String {
    match n.kind {
        Kind::Variable => {
            let ty = n
                .metadata
                .as_ref()
                .and_then(|m| m.lines().next())
                .unwrap_or("Unknown");
            format!(
                "<b>{}</b><br/><font point-size=\"10\">: {}</font>",
                escape(&n.label),
                escape(ty)
            )
        }
        Kind::Function => {
            let sig = n
                .metadata
                .as_ref()
                .and_then(|m| m.lines().next())
                .unwrap_or("");
            format!(
                "<b>{}</b><br/><font point-size=\"10\">{}</font>",
                escape(&n.label),
                escape(sig)
            )
        }
        _ => format!("<b>{}</b>", escape(&n.label)),
    }
}

/// Escapes DOT/HTML-sensitive characters.
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Sanitizes identifiers for DOT cluster names.
fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}
