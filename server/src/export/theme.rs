//! # Export Theme
//!
//! Centralized styling definitions for all static export formats (DOT, Mermaid, TikZ).
//! Ensures visual consistency across different outputs.

use crate::models::{Kind, Node};

pub struct Style<'a> {
    pub fill: &'a str,
    pub border: &'a str,
    pub text: &'a str,
    pub shape: Shape,
    pub latex_color: &'a str, // Name of the predefined latex color
}

pub enum Shape {
    Box,
    Octagon,
    Rounded,
    Diamond,
}

pub struct Theme;

impl Theme {
    /// Returns the style for a given node based on its Kind and properties.
    pub fn get_node_style(node: &Node) -> Style {
        if node.is_artificial_error_termination {
            return Style {
                fill: "#dc2626", // Red 600
                border: "#b91c1c",
                text: "#ffffff",
                shape: Shape::Diamond,
                latex_color: "TectRed",
            };
        }

        if node.is_artificial_graph_start || node.is_artificial_graph_end {
            return Style {
                fill: "#059669", // Emerald 600
                border: "#047857",
                text: "#ffffff",
                shape: Shape::Rounded,
                latex_color: "TectGreen",
            };
        }


        Style {
            fill: "#2563eb", // Blue 600
            border: "#1d4ed8",
            text: "#ffffff",
            shape: Shape::Box,
            latex_color: "TectBlue",
        }
    }

    pub fn get_token_color(kind: &Kind) -> (&'static str, &'static str) {
        match kind {
            Kind::Constant(_) => ("#a855f7", "TectPurple"), // Purple
            Kind::Variable(_) => ("#64748b", "TectGray"),   // Slate
            Kind::Error(_) => ("#ef4444", "TectRed"),       // Red
        }
    }
}
