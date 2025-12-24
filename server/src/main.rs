use anyhow::Result;
use clap::Parser as ClapParser;
use dashmap::DashMap;
use std::fs;
use std::path::PathBuf;
use tower_lsp::{LspService, Server};
use walkdir::WalkDir;

mod analyzer;
mod graphviz;
mod lsp;
mod models;
mod tests;
mod test_parse;

/// The primary entry point for the Tect toolset.
///
/// Tect can be executed in two distinct modes:
/// 1. **CLI Mode**: Triggered by providing an input path. Generates an architectural graph.
/// 2. **LSP Mode**: Default mode when no path is provided. Acts as a Language Server backend.
#[derive(ClapParser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The architectural source file or directory to analyze.
    /// If omitted, the tool starts the Language Server.
    input: Option<PathBuf>,

    /// Specifies the target path to save the generated architectural model.
    /// Supported extensions:
    /// - `.json` (default)
    /// - `.dot`  (Graphviz, optimized for text-heavy nodes)
    #[arg(short, long)]
    output: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args_res = Args::try_parse();

    if let Ok(args) = args_res {
        if let Some(input_path) = args.input {
            let mut analyzer = analyzer::TectAnalyzer::new();

            let files = if input_path.is_dir() {
                WalkDir::new(input_path)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().is_some_and(|ext| ext == "tect"))
                    .map(|e| e.path().to_path_buf())
                    .collect::<Vec<_>>()
            } else {
                vec![input_path]
            };

            for file in files {
                let content = fs::read_to_string(&file)?;
                let _ = analyzer.analyze(&content);
            }

            if let Some(out_path) = args.output {
                match out_path.extension().and_then(|e| e.to_str()) {
                    Some("dot") => {
                        let dot = graphviz::to_dot(&analyzer.graph);
                        fs::write(out_path, dot)?;
                    }
                    _ => {
                        let json = serde_json::to_string_pretty(&analyzer.graph)?;
                        fs::write(out_path, json)?;
                    }
                }
            } else {
                let json = serde_json::to_string_pretty(&analyzer.graph)?;
                println!("{}", json);
            }

            return Ok(());
        }
    }

    // Default: start Language Server
    let (service, socket) = LspService::new(|client| lsp::Backend {
        client,
        document_map: DashMap::new(),
    });

    Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
        .serve(service)
        .await;

    Ok(())
}
