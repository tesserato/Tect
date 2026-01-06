use anyhow::Result;
use clap::{Parser as ClapParser, Subcommand};
use dashmap::DashMap;
use std::fs;
use std::path::PathBuf;
use tower_lsp::{LspService, Server};

mod analyzer;
mod engine;
mod lsp;
mod models;
mod test_engine;
mod test_parser;
mod vis_js;

/// The primary entry point for the Tect toolset.
#[derive(ClapParser)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Standard flag passed by Language Clients for stdio transport.
    /// Marked as global so it can appear after subcommands.
    #[arg(long, global = true)]
    stdio: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Compiles a .tect file into a graph visualization.
    Build {
        /// The architectural source file.
        input: PathBuf,
        /// Output path (.html for interactive, .json for raw data).
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Starts the Tect Language Server.
    Serve,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Determine command: prioritize the stdio flag (standard LSP behavior)
    // or the explicit 'serve' subcommand, otherwise fallback to default.
    let cmd = if cli.stdio {
        Commands::Serve
    } else {
        cli.command.unwrap_or(Commands::Serve)
    };

    match cmd {
        Commands::Build { input, output } => {
            let content = fs::read_to_string(input)?;
            let mut analyzer = analyzer::TectAnalyzer::new();
            let structure = analyzer.analyze(&content)?;
            let mut flow = engine::Flow::new(true);
            let graph = flow.simulate(&structure);

            match output.extension().and_then(|s| s.to_str()) {
                Some("html") => fs::write(output, vis_js::generate_interactive_html(&graph))?,
                _ => fs::write(output, serde_json::to_string_pretty(&graph)?)?,
            }
        }
        Commands::Serve => {
            let (service, socket) = LspService::new(|client| lsp::Backend {
                client,
                document_state: DashMap::new(),
            });

            Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
                .serve(service)
                .await;
        }
    }
    Ok(())
}
