use crate::models::{Graph, Kind};
use std::collections::HashMap;
use std::fmt::Write;

/// Converts the semantic architectural graph into a Graphviz DOT representation.
///
/// This renderer is optimized for text-heavy nodes:
/// - Compact labels for layout stability
/// - Full documentation exposed via tooltips
/// - Groups mapped to Graphviz clusters
pub fn to_dot(graph: &Graph) -> String {
    let mut out = String::new();

    writeln!(out, "digraph Tect {{").unwrap();
    writeln!(out, "  rankdir=LR;").unwrap();
    writeln!(out, "  compound=true;").unwrap();
    writeln!(out, "  node [shape=box, fontname=\"Inter\"];").unwrap();

    // --- Group nodes into clusters ---
    let mut groups: HashMap<&str, Vec<&crate::models::Node>> = HashMap::new();
    for node in &graph.nodes {
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
            let label = compact_label(n);
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

    // --- Render edges ---
    for e in &graph.edges {
        writeln!(
            out,
            "  \"{}\" -> \"{}\" [label=\"{}\"];",
            e.source, e.target, e.relation
        )
        .unwrap();
    }

    writeln!(out, "}}").unwrap();
    out
}

/// Produces a compact HTML-like label suitable for dense graphs.
fn compact_label(n: &crate::models::Node) -> String {
    match n.kind {
        Kind::Function => {
            let subtitle = n
                .metadata
                .as_ref()
                .and_then(|m| m.lines().next())
                .unwrap_or("");
            format!(
                "<b>{}</b><br/><font point-size=\"10\">{}</font>",
                escape(&n.label),
                escape(subtitle)
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

/// Ensures cluster identifiers are DOT-safe.
fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}
