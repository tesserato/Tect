//! # TikZ (LaTeX) Exporter
//!
//! Generates a .tex file using the `graphs`, `graphdrawing`, and `force` libraries.
//! Requires compilation with LuaLaTeX.

use super::theme::{Shape, Theme};
use crate::models::{EdgeRelation, Graph};
use std::collections::HashMap;
use std::fmt::Write;

pub fn export(graph: &Graph) -> String {
    let mut out = String::new();

    // Preamble hint
    writeln!(out, "% Tect Architecture Export").unwrap();
    writeln!(out, "% Compile with: lualatex output.tex").unwrap();
    writeln!(out, "\\documentclass[tikz,border=10pt]{{standalone}}").unwrap();
    writeln!(
        out,
        "\\usetikzlibrary{{graphs, graphdrawing, shapes.geometric}}"
    )
    .unwrap();
    writeln!(out, "\\usegdlibrary{{force}}").unwrap();
    writeln!(out, "").unwrap();
    writeln!(out, "% Tect Color Palette").unwrap();
    writeln!(out, "\\definecolor{{TectBlue}}{{HTML}}{{2563eb}}").unwrap();
    writeln!(out, "\\definecolor{{TectRed}}{{HTML}}{{dc2626}}").unwrap();
    writeln!(out, "\\definecolor{{TectGreen}}{{HTML}}{{059669}}").unwrap();
    writeln!(out, "\\definecolor{{TectPurple}}{{HTML}}{{a855f7}}").unwrap();
    writeln!(out, "\\definecolor{{TectGray}}{{HTML}}{{64748b}}").unwrap();
    writeln!(out, "").unwrap();
    writeln!(out, "\\begin{{document}}").unwrap();
    writeln!(out, "").unwrap();
    writeln!(out, "\\begin{{tikzpicture}}[").unwrap();
    writeln!(out, "  tect_node/.style={{draw=none, fill=TectBlue, text=white, font=\\sffamily\\small, inner sep=6pt, rounded corners=2pt}},").unwrap();
    writeln!(out, "  tect_edge/.style={{draw=gray!50, thick, ->, >=stealth, font=\\sffamily\\tiny, align=center}}").unwrap();
    writeln!(out, "]").unwrap();
    writeln!(out, "").unwrap();
    writeln!(
        out,
        "\\graph [spring layout, node distance=2cm, iterations=500] {{"
    )
    .unwrap();

    // Group nodes
    let mut groups: HashMap<Option<String>, Vec<&crate::models::Node>> = HashMap::new();
    for node in &graph.nodes {
        let group_name = node.function.group.as_ref().map(|g| g.name.clone());
        groups.entry(group_name).or_default().push(node);
    }

    // Nodes
    for (group_opt, nodes) in groups {
        // TikZ graphs library doesn't handle visual clusters (boxes around nodes)
        // easily within the same layout pass without manual tweaking.
        // We will just comment the group structure for now.
        if let Some(name) = group_opt {
            writeln!(out, "  % Group: {}", name).unwrap();
        } else {
            writeln!(out, "  % Global").unwrap();
        }

        for node in nodes {
            let style = Theme::get_node_style(node);
            let shape_tikz = match style.shape {
                Shape::Box => "rectangle",
                Shape::Octagon => "regular polygon, regular polygon sides=8",
                Shape::Rounded => "rectangle, rounded corners=5pt",
                Shape::Diamond => "diamond",
            };

            // N_123 [as="Name", fill=Color, ...]
            writeln!(
                out,
                "  N_{} [as=\"{}\", tect_node, shape={}, fill={}];",
                node.uid, node.function.name, shape_tikz, style.latex_color
            )
            .unwrap();
        }
    }

    writeln!(out, "").unwrap();

    // Edges
    for edge in &graph.edges {
        let (_, color_name) = Theme::get_token_color(&edge.token.kind);
        let style_extra = match edge.relation {
            EdgeRelation::ControlFlow | EdgeRelation::ErrorFlow => ", dashed",
            _ => "",
        };

        writeln!(
            out,
            "  N_{} -> [tect_edge, color={}, edge label=\"{}\"{}] N_{};",
            edge.from_node_uid,
            color_name,
            edge.token.kind.name(),
            style_extra,
            edge.to_node_uid
        )
        .unwrap();
    }

    writeln!(out, "}};").unwrap();
    writeln!(out, "\\end{{tikzpicture}}").unwrap();
    writeln!(out, "\\end{{document}}").unwrap();

    out
}
