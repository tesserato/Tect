use crate::models::{Graph, Kind};
use std::collections::HashMap;
use std::fmt::Write;

/// Force-directed architectural graph with:
/// - semantic edge strength
/// - directional arrows
/// - soft groups
/// - semantic color coding
pub fn to_dot(graph: &Graph) -> String {
    let mut out = String::new();

    writeln!(out, "digraph Tect {{").unwrap();
    writeln!(out, "  layout=neato;").unwrap();
    writeln!(out, "  overlap=false;").unwrap();
    writeln!(out, "  splines=true;").unwrap();
    writeln!(
        out,
        "  node [shape=box, fontname=\"Inter\", style=rounded];"
    )
    .unwrap();

    // Helper: visible nodes only
    let is_visible = |id: &str| {
        graph
            .nodes
            .iter()
            .any(|n| n.id == id && !matches!(n.kind, Kind::Data | Kind::Error))
    };

    // --- Group nodes by architectural group ---
    let mut groups: HashMap<&str, Vec<&crate::models::Node>> = HashMap::new();
    for n in &graph.nodes {
        if matches!(n.kind, Kind::Data | Kind::Error) {
            continue;
        }
        groups.entry(&n.group).or_default().push(n);
    }

    // --- Emit groups + nodes ---
    for (group, nodes) in groups {
        let clustered = group != "global";

        if clustered {
            writeln!(out, "  subgraph cluster_{} {{", sanitize(group)).unwrap();
            writeln!(out, "    label=\"{}\";", group).unwrap();
            writeln!(out, "    labelloc=\"t\";").unwrap();
            writeln!(out, "    labeljust=\"l\";").unwrap();
            writeln!(out, "    color=\"#444444\";").unwrap();
        }

        for n in nodes {
            let label = node_label(n);
            let (fill, border, font) = node_colors(n.kind);

            let extra = match n.kind {
                Kind::Logic => "shape=octagon",
                _ => "",
            };

            writeln!(
                out,
                "    \"{}\" [label=<{}>, fillcolor=\"{}\", color=\"{}\", fontcolor=\"{}\", style=\"filled,rounded\" {}];",
                n.id,
                label,
                fill,
                border,
                font,
                extra
            )
            .unwrap();
        }

        if clustered {
            writeln!(out, "  }}").unwrap();
        }
    }

    // --- Directed edges with force physics ---
    for e in &graph.edges {
        if !is_visible(&e.source) || !is_visible(&e.target) {
            continue;
        }

        let (weight, len, width, style) = edge_physics(&e.relation);

        writeln!(
            out,
            "  \"{}\" -> \"{}\" \
             [weight={}, len={}, penwidth={}, style=\"{}\", constraint=false, color=\"#888888\"];",
            e.source, e.target, weight, len, width, style
        )
        .unwrap();
    }

    writeln!(out, "}}").unwrap();
    out
}

/// Semantic color palette per node kind.
fn node_colors(kind: Kind) -> (&'static str, &'static str, &'static str) {
    match kind {
        Kind::Variable => ("#7ec97e", "#4f9f4f", "#1f3d1f"), // green
        Kind::Function => ("#6b88a6", "#4a657f", "#1e2d3a"), // blue-gray
        Kind::Logic => ("#f2dede", "#cc6666", "#8b2e2e"),    // control
        _ => ("#e0e0e0", "#999999", "#333333"),              // neutral
    }
}

/// Maps architectural semantics to force + visual properties.
fn edge_physics(rel: &str) -> (f32, f32, f32, &'static str) {
    match rel {
        "result_flow" => (10.0, 1.0, 2.5, "solid"),
        "argument_flow" => (4.0, 2.0, 1.5, "solid"),
        "call" => (2.0, 3.0, 1.0, "dotted"),
        "control_flow" | "break" => (1.0, 4.0, 0.8, "dashed"),
        _ => (1.0, 4.0, 0.8, "dotted"),
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
        _ => format!("<b>{}</b>", escape(&n.label)),
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}
