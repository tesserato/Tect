use crate::models::{Graph, Kind};
use std::collections::HashMap;
use std::fmt::Write;

/// Generates a force-directed architectural graph with:
/// - semantic edge strength
/// - directional arrows
/// - soft architectural groups
pub fn to_dot(graph: &Graph) -> String {
    let mut out = String::new();

    writeln!(out, "digraph Tect {{").unwrap();
    writeln!(out, "  layout=neato;").unwrap();
    writeln!(out, "  overlap=false;").unwrap();
    writeln!(out, "  splines=true;").unwrap();
    writeln!(out, "  node [shape=box, fontname=\"Inter\"];").unwrap();

    // --- Group nodes by architectural group ---
    let mut groups: HashMap<&str, Vec<&crate::models::Node>> = HashMap::new();

    for n in &graph.nodes {
        if matches!(n.kind, Kind::Data | Kind::Error) {
            continue;
        }
        groups.entry(&n.group).or_default().push(n);
    }

    // --- Emit grouped nodes ---
    for (group, nodes) in groups {
        let clustered = group != "global";

        if clustered {
            writeln!(out, "  subgraph cluster_{} {{", sanitize(group)).unwrap();
            writeln!(out, "    label=\"{}\";", group).unwrap();
            writeln!(out, "    style=rounded;").unwrap();
            writeln!(out, "    color=\"#444444\";").unwrap();
        }

        for n in nodes {
            let label = node_label(n);

            let extra = match n.kind {
                Kind::Logic => "shape=octagon,color=\"#cc6666\",fontcolor=\"#cc6666\"",
                _ => "",
            };

            writeln!(out, "    \"{}\" [label=<{}> {}];", n.id, label, extra).unwrap();
        }

        if clustered {
            writeln!(out, "  }}").unwrap();
        }
    }

    // --- Directed edges with force physics ---
    for e in &graph.edges {
        let (weight, len, width, style) = edge_physics(&e.relation);

        writeln!(
            out,
            "  \"{}\" -> \"{}\" \
             [weight={}, len={}, penwidth={}, style=\"{}\", constraint=false];",
            e.source, e.target, weight, len, width, style
        )
        .unwrap();
    }

    writeln!(out, "}}").unwrap();
    out
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

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}
