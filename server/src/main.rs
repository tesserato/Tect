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

#[cfg(test)]
mod tests;

#[derive(ClapParser)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    #[arg(long, global = true)]
    stdio: bool,
}

#[derive(Subcommand)]
enum Commands {
    Build {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
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
        Commands::Build { input, output } => {
            let content = fs::read_to_string(input)?;
            let mut analyzer = analyzer::TectAnalyzer::new();
            let structure = analyzer.analyze(&content);
            let mut flow = engine::Flow::new(true);
            let graph = flow.simulate(&structure);

            match output.extension().and_then(|s| s.to_str()) {
                Some("html") => fs::write(output, vis_js::generate_interactive_html(&graph))?,
                _ => fs::write(output, serde_json::to_string_pretty(&graph)?)?,
            }
        }
        Commands::Serve => {
            let (service, socket) = LspService::build(|client| lsp::Backend {
                client,
                document_state: DashMap::new(),
            })
            // Register custom request handler
            .custom_method("tect/getGraph", lsp::Backend::get_visual_graph)
            .finish();

            Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
                .serve(service)
                .await;
        }
    }
    Ok(())
}
