//! # Mermaid.js Exporter

use super::theme::{Shape, Theme, GROUP_PALETTE};
use crate::models::{EdgeRelation, Graph};
use std::collections::HashMap;
use std::fmt::Write;

pub fn export(graph: &Graph) -> String {
    let mut out = String::new();

    writeln!(out, "flowchart TD").unwrap();

    // Define Base Classes
    writeln!(
        out,
        "    classDef default fill:#1e293b,stroke:#334155,color:#fff;"
    )
    .unwrap();
    writeln!(
        out,
        "    classDef startend fill:#059669,stroke:#047857,color:#fff;"
    )
    .unwrap();
    writeln!(
        out,
        "    classDef error fill:#dc2626,stroke:#b91c1c,color:#fff;"
    )
    .unwrap();
    writeln!(
        out,
        "    classDef function fill:#2563eb,stroke:#1d4ed8,color:#fff;"
    )
    .unwrap();

    // Generate Dynamic Classes for Groups
    // Fill is Blue (#2563eb), Border is Group Color, Thick Stroke
    for (i, hex) in GROUP_PALETTE.iter().enumerate() {
        writeln!(
            out,
            "    classDef group{} fill:#2563eb,stroke:{},stroke-width:3px,color:#fff;",
            i, hex
        )
        .unwrap();
    }

    // Group nodes
    let mut groups: HashMap<Option<String>, Vec<&crate::models::Node>> = HashMap::new();
    for node in &graph.nodes {
        let group_name = node.function.group.as_ref().map(|g| g.name.clone());
        groups.entry(group_name).or_default().push(node);
    }

    for (group_opt, nodes) in groups {
        let is_cluster = group_opt.is_some();

        if let Some(group_name) = group_opt {
            writeln!(out, "    subgraph {}", sanitize_id(&group_name)).unwrap();
            writeln!(out, "        direction TB").unwrap();
        }

        for node in nodes {
            let style = Theme::get_node_style(node);
            let shape_open = match style.shape {
                Shape::Box => "[",
                Shape::Rounded => "(",
                Shape::Diamond => "{",
            };
            let shape_close = match style.shape {
                Shape::Box => "]",
                Shape::Rounded => ")",
                Shape::Diamond => "}",
            };

            // Node Definition
            writeln!(
                out,
                "        N_{}{}\"{}\"{}",
                node.uid, shape_open, node.function.name, shape_close
            )
            .unwrap();

            // Assign Class
            let class_name = if style.latex_border.starts_with("TectGroup") {
                // Map TectGroupX -> groupX
                style.latex_border.replace("TectGroup", "group")
            } else if node.is_artificial_error_termination {
                "error".to_string()
            } else if node.is_artificial_graph_start || node.is_artificial_graph_end {
                "startend".to_string()
            } else {
                "function".to_string()
            };

            writeln!(out, "        class N_{} {}", node.uid, class_name).unwrap();
        }

        if is_cluster {
            writeln!(out, "    end").unwrap();
        }
    }

    // Edges
    for edge in &graph.edges {
        let arrow = match edge.relation {
            EdgeRelation::ErrorFlow => "-.->",
            EdgeRelation::ControlFlow => "-.->",
            _ => "-->",
        };

        writeln!(
            out,
            "    N_{} {}|{}| N_{}",
            edge.from_node_uid,
            arrow,
            edge.token.kind.name(),
            edge.to_node_uid
        )
        .unwrap();
    }

    out
}

fn sanitize_id(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}
