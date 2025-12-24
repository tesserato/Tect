use crate::models::{Graph, Kind};
use std::fmt::Write;

/// Generates a force-directed Graphviz DOT graph.
///
/// The layout is driven by semantic edge strength so that
/// nodes self-organize based on interaction density.
pub fn to_dot(graph: &Graph) -> String {
    let mut out = String::new();

    writeln!(out, "graph Tect {{").unwrap();
    writeln!(out, "  layout=neato;").unwrap();
    writeln!(out, "  overlap=false;").unwrap();
    writeln!(out, "  splines=true;").unwrap();
    writeln!(out, "  node [shape=box, fontname=\"Inter\"];").unwrap();

    // --- Nodes ---
    for n in &graph.nodes {
        // Hide pure type definitions
        if matches!(n.kind, Kind::Data | Kind::Error) {
            continue;
        }

        let label = node_label(n);

        let extra = match n.kind {
            Kind::Logic => "shape=octagon,color=\"#cc6666\",fontcolor=\"#cc6666\"",
            _ => "",
        };

        writeln!(out, "  \"{}\" [label=<{}> {}];", n.id, label, extra).unwrap();
    }

    // --- Edges (force-driven) ---
    for e in &graph.edges {
        let (weight, len, width) = edge_physics(&e.relation);

        writeln!(
            out,
            "  \"{}\" -- \"{}\" [weight={}, len={}, penwidth={}, constraint=false];",
            e.source, e.target, weight, len, width
        )
        .unwrap();
    }

    writeln!(out, "}}").unwrap();
    out
}

/// Maps architectural semantics to physical force parameters.
fn edge_physics(rel: &str) -> (f32, f32, f32) {
    match rel {
        "result_flow" => (10.0, 1.0, 2.5),
        "argument_flow" => (4.0, 2.0, 1.5),
        "call" => (2.0, 3.0, 1.0),
        "control_flow" | "break" => (1.0, 4.0, 0.8),
        _ => (1.0, 4.0, 0.8),
    }
}

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
        Kind::Function => format!("<b>{}</b>", escape(&n.label)),
        Kind::Logic => format!("<b>{}</b>", escape(&n.label)),
        _ => format!("<b>{}</b>", escape(&n.label)),
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
