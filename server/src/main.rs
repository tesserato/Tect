use anyhow::Result;
use clap::Parser as ClapParser;
use dashmap::DashMap;
use std::fs;
use std::path::PathBuf;
use tower_lsp::{LspService, Server};
use walkdir::WalkDir;

mod analyzer;
mod lsp;
mod models;
mod tests;

#[derive(ClapParser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    input: Option<PathBuf>,
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
                    .filter(|e| e.path().extension().map_or(false, |ext| ext == "tect"))
                    .map(|e| e.path().to_path_buf())
                    .collect::<Vec<_>>()
            } else { 
                vec![input_path] 
            };

            for file in files {
                let content = fs::read_to_string(&file)?;
                let _ = analyzer.analyze(&content); 
            }
            
            let json_output = serde_json::to_string_pretty(&analyzer.graph)?;
            if let Some(out_path) = args.output { 
                fs::write(out_path, json_output)?; 
            } else { 
                println!("{}", json_output); 
            }
            return Ok(());
        }
    }

    let (service, socket) = LspService::new(|client| lsp::Backend { 
        client, 
        document_map: DashMap::new() 
    });
    Server::new(tokio::io::stdin(), tokio::io::stdout(), socket).serve(service).await;
    Ok(())
}