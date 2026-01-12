//! # Export Theme
//!
//! Centralized styling definitions for all static export formats (DOT, Mermaid, TikZ).
//! Ensures visual consistency across different outputs.

use crate::models::{Kind, Node};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Hex colors for the group palette.
/// High contrast, distinct colors chosen for visibility against dark backgrounds.
/// Excludes Status Colors: Red (Error), Emerald (Start/End).
pub const GROUP_PALETTE: &[&str] = &[
    "#2563eb", // Blue 600
    "#d97706", // Amber 600
    "#7c3aed", // Violet 600
    "#db2777", // Pink 600
    "#0891b2", // Cyan 600
    "#ea580c", // Orange 600
    "#4f46e5", // Indigo 600
    "#c026d3", // Fuchsia 600
    "#0284c7", // Sky 600
    "#ca8a04", // Yellow 600
];

pub struct Style {
    pub fill: String,
    pub border: String,
    pub text: String,
    pub shape: Shape,
    pub latex_fill: String,
    pub latex_border: String,
    pub stroke_width: u32,
}

pub enum Shape {
    Box,
    Rounded,
    Diamond,
}

pub struct Theme;

impl Theme {
    /// Returns the (Hex, LatexName) tuple for a specific group name.
    pub fn get_group_color(name: &str) -> (String, String) {
        let idx = get_palette_index(name);
        (GROUP_PALETTE[idx].to_string(), format!("TectGroup{}", idx))
    }

    /// Returns the style for a given node based on its Kind and properties.
    pub fn get_node_style(node: &Node) -> Style {
        if node.is_artificial_error_termination {
            return Style {
                fill: "#dc2626".into(), // Red 600
                border: "#b91c1c".into(),
                text: "#ffffff".into(),
                shape: Shape::Diamond,
                latex_fill: "TectRed".into(),
                latex_border: "TectRedDark".into(),
                stroke_width: 1,
            };
        }

        if node.is_artificial_graph_start || node.is_artificial_graph_end {
            return Style {
                fill: "#059669".into(), // Emerald 600
                border: "#047857".into(),
                text: "#ffffff".into(),
                shape: Shape::Rounded,
                latex_fill: "TectGreen".into(),
                latex_border: "TectGreenDark".into(),
                stroke_width: 1,
            };
        }

        // Logic for Standard Functions
        if let Some(group) = &node.function.group {
            // Grouped Node: Blue Body, Group-Colored Border
            let (group_hex, group_latex) = Self::get_group_color(&group.name);

            Style {
                fill: "#1e293b".into(), // Slate 800 (Darker body for contrast)
                border: group_hex,      // Dynamic Group Color
                text: "#ffffff".into(),
                shape: Shape::Box,
                latex_fill: "TectBlue".into(),
                latex_border: group_latex,
                stroke_width: 3, // Thick border to emphasize affiliation
            }
        } else {
            // Ungrouped Node: Blue Body, Dark Blue Border
            Style {
                fill: "#1e293b".into(),
                border: "#475569".into(), // Slate 600
                text: "#ffffff".into(),
                shape: Shape::Box,
                latex_fill: "TectBlue".into(),
                latex_border: "TectBlueDark".into(),
                stroke_width: 1,
            }
        }
    }

    pub fn get_token_color(kind: &Kind) -> (&'static str, &'static str) {
        match kind {
            Kind::Constant(_) => ("#a855f7", "TectPurple"), // Purple
            Kind::Variable(_) => ("#94a3b8", "TectGray"),   // Slate 400
            Kind::Error(_) => ("#ef4444", "TectRed"),       // Red
        }
    }
}

fn get_palette_index(s: &str) -> usize {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    (hasher.finish() as usize) % GROUP_PALETTE.len()
}
