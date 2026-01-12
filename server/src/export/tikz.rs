//! # TikZ (LaTeX) Exporter

use super::theme::{Shape, Theme, GROUP_PALETTE};
use crate::models::{EdgeRelation, Graph};
use std::collections::HashMap;
use std::fmt::Write;

pub fn export(graph: &Graph) -> String {
    let mut out = String::new();

    writeln!(out, "% Tect Architecture Export").unwrap();
    writeln!(out, "% Compile with: lualatex output.tex").unwrap();
    writeln!(out, "\\documentclass[tikz,border=10pt]{{standalone}}").unwrap();
    writeln!(
        out,
        "\\usetikzlibrary{{graphs, graphdrawing, shapes.geometric}}"
    )
    .unwrap();
    writeln!(out, "\\usegdlibrary{{force}}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "% Tect Color Palette").unwrap();
    writeln!(out, "\\definecolor{{TectBlue}}{{HTML}}{{2563eb}}").unwrap();
    writeln!(out, "\\definecolor{{TectBlueDark}}{{HTML}}{{1d4ed8}}").unwrap();
    writeln!(out, "\\definecolor{{TectRed}}{{HTML}}{{dc2626}}").unwrap();
    writeln!(out, "\\definecolor{{TectRedDark}}{{HTML}}{{b91c1c}}").unwrap();
    writeln!(out, "\\definecolor{{TectGreen}}{{HTML}}{{059669}}").unwrap();
    writeln!(out, "\\definecolor{{TectGreenDark}}{{HTML}}{{047857}}").unwrap();
    writeln!(out, "\\definecolor{{TectPurple}}{{HTML}}{{a855f7}}").unwrap();
    writeln!(out, "\\definecolor{{TectGray}}{{HTML}}{{64748b}}").unwrap();
    // Dynamic Group Colors
    for (i, hex) in GROUP_PALETTE.iter().enumerate() {
        writeln!(
            out,
            "\\definecolor{{TectGroup{}}}{{HTML}}{{{}}}",
            i,
            hex.trim_start_matches('#')
        )
        .unwrap();
    }
    writeln!(out).unwrap();
    writeln!(out, "\\begin{{document}}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "\\begin{{tikzpicture}}[").unwrap();
    writeln!(out, "  tect_node/.style={{draw=none, text=white, font=\\sffamily\\small, inner sep=6pt, rounded corners=2pt}},").unwrap();
    writeln!(out, "  tect_edge/.style={{draw=gray!50, thick, ->, >=stealth, font=\\sffamily\\tiny, align=center}}").unwrap();
    writeln!(out, "]").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "\\graph [spring layout, node distance=2cm, iterations=500] {{"
    )
    .unwrap();

    let mut groups: HashMap<Option<String>, Vec<&crate::models::Node>> = HashMap::new();
    for node in &graph.nodes {
        let group_name = node.function.group.as_ref().map(|g| g.name.clone());
        groups.entry(group_name).or_default().push(node);
    }

    for (group_opt, nodes) in groups {
        if let Some(name) = group_opt {
            writeln!(out, "  % Group: {}", name).unwrap();
        } else {
            writeln!(out, "  % Global").unwrap();
        }

        for node in nodes {
            let style = Theme::get_node_style(node);
            let shape_tikz = match style.shape {
                Shape::Box => "rectangle",
                Shape::Rounded => "rectangle, rounded corners=5pt",
                Shape::Diamond => "diamond",
            };

            // Apply fill and stroke colors from theme
            // latex_fill is used for 'fill', latex_border is used for 'draw'
            let draw_opts = if style.stroke_width > 0 {
                format!(
                    ", draw={}, line width={}pt",
                    style.latex_border, style.stroke_width
                )
            } else {
                "".to_string()
            };

            writeln!(
                out,
                "  N_{} [as=\"{}\", tect_node, shape={}, fill={}{}];",
                node.uid, node.function.name, shape_tikz, style.latex_fill, draw_opts
            )
            .unwrap();
        }
    }

    writeln!(out).unwrap();

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
