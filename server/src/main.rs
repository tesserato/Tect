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
mod vis_js;

mod test_engine;
mod test_parser;

#[derive(ClapParser)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Compiles a .tect file into a graph visualization.
    Build {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Starts the Tect Language Server (default).
    Serve,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command.unwrap_or(Commands::Serve) {
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
