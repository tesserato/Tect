//! # Tect Code Formatter
//!
//! Responsible for standardizing the visual style of Tect source files.
//! Handles indentation, spacing, and token list normalization.

use crate::analyzer::{Rule, TectParser};
use pest::Parser;

/// Represents a formatted block of code.
///
/// A block corresponds to a logical unit in the source (e.g., a function definition,
/// a comment, or an import statement) that has been individually formatted.
struct Block {
    /// The formatted text content of the block.
    content: String,
    /// The start byte position in the original source.
    start_pos: usize,
    /// The end byte position in the original source.
    end_pos: usize,
}

/// Formats the given Tect source code string.
///
/// This function parses the input code, standardizes the formatting of individual components
/// (like functions and token lists), and reconstructs the file. It attempts to preserve
/// vertical spacing (blank lines) from the original source where appropriate.
///
/// # Returns
/// `Some(String)` containing the formatted code if parsing succeeds, or `None` if parsing fails.
pub fn format_tect_source(content: &str) -> Option<String> {
    let mut blocks = Vec::new();

    let parsed = match TectParser::parse(Rule::program, content) {
        Ok(mut p) => p.next().unwrap(),
        Err(_) => return None,
    };

    // 1. Blockify: Convert AST pairs into styled Blocks
    for pair in parsed.into_inner() {
        if pair.as_rule() == Rule::EOI {
            continue;
        }

        let span = pair.as_span();
        let formatted_content = match pair.as_rule() {
            Rule::func_def => format_function(pair),
            Rule::import_stmt => pair.as_str().trim().to_string(),
            Rule::comment | Rule::flow_step => pair.as_str().trim().to_string(),
            _ => pair.as_str().trim().to_string(), // Constants, vars, etc.
        };

        if !formatted_content.is_empty() {
            blocks.push(Block {
                content: formatted_content,
                start_pos: span.start(),
                end_pos: span.end(),
            });
        }
    }

    if blocks.is_empty() {
        return Some(String::new());
    }

    // 2. Glue: Join blocks based on original source whitespace
    let mut result = String::new();
    result.push_str(&blocks[0].content);

    for i in 1..blocks.len() {
        let prev = &blocks[i - 1];
        let curr = &blocks[i];

        // Check original source between end of prev and start of curr
        let gap = &content[prev.end_pos..curr.start_pos];
        let newline_count = gap.chars().filter(|&c| c == '\n').count();

        // If gap has >= 2 newlines (a blank line), force \n\n.
        // Else use \n (this keeps comments attached to code if they were adjacent).
        let separator = if newline_count >= 2 { "\n\n" } else { "\n" };

        result.push_str(separator);
        result.push_str(&curr.content);
    }

    result.push('\n');
    Some(result)
}

/// Formats a list of tokens (e.g., in a function signature) into a standard string representation.
///
/// Example: converts `[A, B, C]` or `A, B` into a comma-separated string `A, B, C`.
fn format_token_list(pair: pest::iterators::Pair<Rule>) -> String {
    pair.into_inner()
        .map(|t| {
            // t is Rule::token -> inner is Rule::collection or Rule::unitary
            let inner = t.into_inner().next().unwrap();
            match inner.as_rule() {
                Rule::collection => {
                    let name = inner.into_inner().next().unwrap().as_str().trim();
                    format!("[{}]", name)
                }
                _ => inner.as_str().trim().to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Formats a function definition node.
///
/// This rebuilds the function definition from its parts:
/// 1. Documentation comments.
/// 2. Header (Group, Function Keyword, Name, Input tokens).
/// 3. Output sections (indented).
fn format_function(pair: pest::iterators::Pair<Rule>) -> String {
    let mut inner = pair.clone().into_inner();
    let mut parts = Vec::new();
    let mut last_inner_pos = None;

    // 1. Extract Docs
    while let Some(p) = inner.peek() {
        let pos = p.as_span().start();
        if last_inner_pos == Some(pos) {
            break;
        }
        last_inner_pos = Some(pos);
        if p.as_rule() == Rule::doc_line {
            parts.push(inner.next().unwrap().as_str().trim().to_string());
        } else {
            break;
        }
    }

    // 2. Extract Header (Group, Function Keyword, Name, Inputs)
    let mut header = Vec::new();
    while let Some(p) = inner.peek() {
        if p.as_rule() == Rule::token_list {
            header.push(format_token_list(inner.next().unwrap()));
        } else if matches!(p.as_rule(), Rule::ident | Rule::kw_function) {
            header.push(inner.next().unwrap().as_str().trim().to_string());
        } else {
            break;
        }
    }
    if !header.is_empty() {
        parts.push(header.join(" "));
    }

    // 3. Extract Outputs (Indented)
    if let Some(p) = inner.next() {
        for child in p.into_inner() {
            if child.as_rule() == Rule::output_line {
                let raw = child.as_str().trim();
                let symbol = if raw.starts_with('>') { ">" } else { "|" };
                let mut output_parts = child.into_inner();
                // Token list
                let tokens = output_parts
                    .next()
                    .map(format_token_list)
                    .unwrap_or_default();
                parts.push(format!("    {} {}", symbol, tokens));
            }
        }
    }

    parts.join("\n")
}
