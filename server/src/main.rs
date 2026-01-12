use anyhow::{Context, Result};
use clap::{Parser as ClapParser, Subcommand};
use colored::Colorize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tower_lsp::lsp_types::{DiagnosticSeverity, Url};
use tower_lsp::{LspService, Server};

use crate::export::vis_js;

mod analyzer;
mod engine;
mod export;
mod formatter;
mod lsp;
mod models;
mod source_manager;
// mod vis_js;

#[cfg(test)]
mod tests;

/// Tect: Architectural Specification Language & Visualizer.
///
/// Tect is a tool for defining software architectures using a lightweight,
/// type-safe language. It simulates data flow, detects architectural errors
/// (like cycles or starvation), and exports diagrams to various formats.
///
/// =========================================================================
/// COMMON WORKFLOWS
/// =========================================================================
///
/// 1. Define architecture in .tect files
///    (See examples in the repo for syntax)
///
/// 2. Verify logic (Check for starvation, cycles, unused symbols):
///    $ tect check main.tect
///
/// 3. Generate diagrams (Export to standard formats):
///    $ tect build main.tect -o arch.html  (Interactive HTML with Physics)
///    $ tect build main.tect -o arch.mmd   (Mermaid for Markdown)
///    $ tect build main.tect -o arch.tex   (LaTeX/TikZ for Papers)
///
/// =========================================================================
///
#[derive(ClapParser)]
#[command(name = "tect")]
#[command(author = "Tesserato")]
#[command(version = "0.0.4")]
#[command(propagate_version = true)]
#[command(verbatim_doc_comment)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Force stdio mode (internal use for LSP communication)
    #[arg(long, global = true, hide = true)]
    stdio: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile to HTML, DOT, Mermaid, LaTeX, or JSON.
    ///
    /// The output format is automatically determined by the output file extension.
    ///
    /// [SUPPORTED FORMATS]
    ///
    /// .html   Interactive Web Graph (Vis.js)
    ///         Best for: Exploring complex architectures with physics.
    ///         Features: Search, Physics toggles, Clustering.
    ///
    /// .mmd    Mermaid.js Diagram
    ///         Best for: Embedding in GitHub/GitLab READMEs, Notion, or Obsidian.
    ///         Renders directly in many markdown previewers.
    ///
    /// .tex    TikZ/LaTeX (LuaLaTeX)
    ///         Best for: Publication-quality PDF documents and academic papers.
    ///         Uses the `force` graph library for auto-layout.
    ///
    /// .dot    Graphviz DOT
    ///         Best for: Interoperability with Graphviz tools (dot, neato, fdp).
    ///         Standard format for graph processing.
    ///
    /// .json   Raw Data
    ///         Best for: Custom tooling or programmatic analysis.
    ///         Contains the full node/edge list and group metadata.
    #[command(visible_alias = "b")]
    #[command(verbatim_doc_comment)]
    Build {
        /// The input .tect file
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// The output file path.
        /// The extension (.html, .mmd, .tex, .dot, .json) determines the format.
        #[arg(short, long, value_name = "OUTPUT")]
        output: PathBuf,
    },

    /// Auto-format Tect source to standard style.
    ///
    /// Standardizes indentation (4 spaces), aligns comments, and normalizes
    /// token lists. By default, this command overwrites the input file.
    #[command(visible_alias = "f")]
    Fmt {
        /// The input .tect file
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// Write to a specific output path instead of overwriting the input.
        #[arg(short, long, value_name = "OUTPUT")]
        output: Option<PathBuf>,
    },

    /// Check source for syntax and logic errors.
    ///
    /// Performs a full compiler pass:
    /// 1. Syntax Parsing (Grammar validation)
    /// 2. Semantic Analysis (Symbol resolution, Cycle detection)
    /// 3. Flow Simulation (Starvation detection, Dead-end detection)
    ///
    /// Prints colored diagnostics with file locations.
    #[command(visible_alias = "c")]
    #[command(verbatim_doc_comment)]
    Check {
        /// The input .tect file
        #[arg(value_name = "INPUT")]
        input: PathBuf,
    },

    /// Start the Language Server (LSP).
    ///
    /// Used by editor extensions (VS Code, Neovim) to provide autocomplete,
    /// hover documentation, and live error highlighting.
    /// Do not run this manually unless debugging the LSP protocol.
    Serve,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let cmd = if cli.stdio {
        Commands::Serve
    } else {
        cli.command.unwrap_or(Commands::Serve)
    };

    match cmd {
        Commands::Build { input, output } => handle_build(input, output),
        Commands::Fmt { input, output } => handle_fmt(input, output),
        Commands::Check { input } => handle_check(input),
        Commands::Serve => handle_serve().await,
    }
}

fn handle_build(input: PathBuf, output: PathBuf) -> Result<()> {
    let content = fs::read_to_string(&input).context("Failed to read input file")?;
    let abs_path = fs::canonicalize(&input).unwrap_or(input.clone());
    let root_uri =
        Url::from_file_path(abs_path).map_err(|_| anyhow::anyhow!("Invalid file path"))?;

    // 1. Analyze
    let mut workspace = analyzer::Workspace::new();
    workspace.analyze(root_uri, Some(content));

    // 2. Simulate
    let mut flow = engine::Flow::new(true);
    let graph = flow.simulate(&workspace.structure);

    // 3. Export based on extension
    let extension = output
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("json");

    match extension {
        "html" => {
            let html = vis_js::generate_interactive_html(&graph);
            fs::write(&output, html)?;
            println!("{} HTML: {:?}", "Success:".green().bold(), output);
        }
        "dot" | "gv" => {
            let content = export::dot::export(&graph);
            fs::write(&output, content)?;
            println!("{} DOT: {:?}", "Success:".green().bold(), output);
        }
        "mmd" | "mermaid" => {
            let content = export::mermaid::export(&graph);
            fs::write(&output, content)?;
            println!("{} Mermaid: {:?}", "Success:".green().bold(), output);
        }
        "tex" => {
            let content = export::tikz::export(&graph);
            fs::write(&output, content)?;
            println!("{} TikZ/LaTeX: {:?}", "Success:".green().bold(), output);
        }
        _ => {
            let json = serde_json::to_string_pretty(&graph)?;
            fs::write(&output, json)?;
            println!("{} JSON: {:?}", "Success:".green().bold(), output);
        }
    }

    Ok(())
}

fn handle_fmt(input: PathBuf, output: Option<PathBuf>) -> Result<()> {
    let content = fs::read_to_string(&input).context("Failed to read input file")?;

    match formatter::format_tect_source(&content) {
        Some(formatted) => {
            let target = output.unwrap_or(input);
            fs::write(&target, formatted).context("Failed to write formatted output")?;
            println!("{} Formatted {:?}", "Success:".green().bold(), target);
            Ok(())
        }
        None => {
            eprintln!(
                "{} Failed to parse file for formatting. Check syntax errors.",
                "Error:".red().bold()
            );
            std::process::exit(1);
        }
    }
}

fn handle_check(input: PathBuf) -> Result<()> {
    let content = fs::read_to_string(&input).context("Failed to read input file")?;
    let abs_path = fs::canonicalize(&input).unwrap_or(input.clone());
    let root_uri =
        Url::from_file_path(abs_path).map_err(|_| anyhow::anyhow!("Invalid file path"))?;

    let mut workspace = analyzer::Workspace::new();
    workspace.analyze(root_uri, Some(content));

    // Run engine only if no fatal parsing errors
    let has_fatal = workspace
        .structure
        .diagnostics
        .iter()
        .any(|d| d.severity == DiagnosticSeverity::ERROR);

    if !has_fatal {
        let mut flow = engine::Flow::new(true);
        let _graph = flow.simulate(&workspace.structure);
        workspace.structure.diagnostics.extend(flow.diagnostics);
    }

    let diagnostics = &workspace.structure.diagnostics;

    if diagnostics.is_empty() {
        println!("{} No issues found.", "Success:".green().bold());
        return Ok(());
    }

    let mut err_count = 0;
    let mut warn_count = 0;

    for diag in diagnostics {
        let severity_label = match diag.severity {
            DiagnosticSeverity::ERROR => {
                err_count += 1;
                "Error".red().bold()
            }
            DiagnosticSeverity::WARNING => {
                warn_count += 1;
                "Warning".yellow().bold()
            }
            DiagnosticSeverity::INFORMATION => "Info".blue().bold(),
            DiagnosticSeverity::HINT => "Hint".cyan(),
            _ => "Diagnostic".white(),
        };

        let location_str = if let Some(span) = diag.span {
            let range = workspace.source_manager.resolve_range(span);
            format!(
                "{}:{}:{}",
                workspace
                    .source_manager
                    .get_uri(span.file_id)
                    .and_then(|u| u.to_file_path().ok())
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                    .unwrap_or_else(|| "unknown".to_string()),
                range.start.line + 1,
                range.start.character + 1
            )
        } else {
            "global".to_string()
        };

        println!(
            "{}: {} {}",
            severity_label,
            format!("[{}]", location_str).dimmed(),
            diag.message
        );
    }

    println!();
    if err_count > 0 {
        eprintln!(
            "{} Found {} errors, {} warnings.",
            "Failure:".red().bold(),
            err_count,
            warn_count
        );
        std::process::exit(1);
    } else {
        println!(
            "{} Found {} errors, {} warnings.",
            "Success:".green().bold(),
            err_count,
            warn_count
        );
    }

    Ok(())
}

async fn handle_serve() -> Result<()> {
    let (service, socket) = LspService::build(|client| lsp::Backend {
        client,
        workspace: Mutex::new(analyzer::Workspace::new()),
        open_documents: Mutex::new(HashSet::new()),
        graph_cache: Mutex::new(HashMap::new()),
    })
    .custom_method("tect/getGraph", lsp::Backend::get_visual_graph)
    .custom_method("tect/exportGraph", lsp::Backend::get_export_content)
    .finish();

    Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
        .serve(service)
        .await;
    Ok(())
}
