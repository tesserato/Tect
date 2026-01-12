use anyhow::Result;
use clap::{Parser as ClapParser, Subcommand};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tower_lsp::lsp_types::Url;
use tower_lsp::{LspService, Server};

mod analyzer;
mod engine;
mod formatter;
mod lsp;
mod models;
mod source_manager;
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
            let content = fs::read_to_string(&input)?;

            // Canonicalize to absolute path then convert to file:// URL
            let abs_path = fs::canonicalize(&input)?;
            let root_uri = Url::from_file_path(abs_path).expect("Invalid file path");

            // Initialize Workspace (Analyzer + VFS)
            let mut workspace = analyzer::Workspace::new();

            // Analyze the source using the URL
            workspace.analyze(root_uri, Some(content));

            // Run the Logic Engine on the resulting IR
            let mut flow = engine::Flow::new(true);
            let graph = flow.simulate(&workspace.structure);

            match output.extension().and_then(|s| s.to_str()) {
                Some("html") => fs::write(output, vis_js::generate_interactive_html(&graph))?,
                _ => fs::write(output, serde_json::to_string_pretty(&graph)?)?,
            }
        }
        Commands::Serve => {
            let (service, socket) = LspService::build(|client| lsp::Backend {
                client,
                workspace: Mutex::new(analyzer::Workspace::new()),
                open_documents: Mutex::new(HashSet::new()),
            })
            // Register custom request handler for the Visualizer
            .custom_method("tect/getGraph", lsp::Backend::get_visual_graph)
            .finish();

            Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
                .serve(service)
                .await;
        }
    }
    Ok(())
}
