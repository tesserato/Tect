//! # Graphviz (DOT) Exporter

use super::theme::{Shape, Theme};
use crate::models::{EdgeRelation, Graph};
use std::collections::HashMap;
use std::fmt::Write;

pub fn export(graph: &Graph) -> String {
    let mut out = String::new();

    writeln!(out, "digraph Tect {{").unwrap();
    writeln!(out, "    layout=dot;").unwrap();
    writeln!(out, "    rankdir=TD;").unwrap();
    writeln!(out, "    node [fontname=\"Helvetica\", fontsize=10];").unwrap();
    writeln!(out, "    edge [fontname=\"Helvetica\", fontsize=9];").unwrap();

    let mut groups: HashMap<Option<String>, Vec<&crate::models::Node>> = HashMap::new();
    for node in &graph.nodes {
        let group_name = node.function.group.as_ref().map(|g| g.name.clone());
        groups.entry(group_name).or_default().push(node);
    }

    for (group_opt, nodes) in groups {
        let is_cluster = group_opt.is_some();

        if let Some(group_name) = group_opt {
            writeln!(out, "    subgraph cluster_{} {{", sanitize_id(&group_name)).unwrap();
            writeln!(out, "        label=\"{}\";", group_name).unwrap();
            writeln!(out, "        style=rounded;").unwrap();
            writeln!(out, "        color=\"#94a3b8\";").unwrap();
            writeln!(out, "        fontcolor=\"#475569\";").unwrap();
        }

        for node in nodes {
            let style = Theme::get_node_style(node);
            let shape_str = match style.shape {
                Shape::Box => "box",
                Shape::Rounded => "rect, style=\"rounded,filled\"",
                Shape::Diamond => "diamond",
            };

            let label = format!("<<B>{}</B>>", escape_html(&node.function.name));

            let style_attr = if shape_str.contains("style=") {
                ""
            } else {
                ", style=filled"
            };

            // Apply style from theme
            writeln!(
                out,
                "        N_{} [label={}, shape={}, fillcolor=\"{}\", color=\"{}\", penwidth={}, fontcolor=\"{}\"{}];",
                node.uid,
                label,
                shape_str,
                style.fill,
                style.border, // Group Color
                style.stroke_width, // Thick stroke for groups
                style.text,
                style_attr
            )
            .unwrap();
        }

        if is_cluster {
            writeln!(out, "    }}").unwrap();
        }
    }

    for edge in &graph.edges {
        let (color, _) = Theme::get_token_color(&edge.token.kind);
        let style = match edge.relation {
            EdgeRelation::ControlFlow => "dashed",
            EdgeRelation::ErrorFlow => "dotted",
            _ => "solid",
        };

        writeln!(
            out,
            "    N_{} -> N_{} [label=\"{}\", color=\"{}\", style=\"{}\"];",
            edge.from_node_uid,
            edge.to_node_uid,
            edge.token.kind.name(),
            color,
            style
        )
        .unwrap();
    }

    writeln!(out, "}}").unwrap();
    out
}

fn sanitize_id(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
