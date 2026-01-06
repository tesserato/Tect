use anyhow::Result;
use clap::Parser as ClapParser;
use dashmap::DashMap;
use std::fs;
use std::path::PathBuf;
use tower_lsp::{LspService, Server};
use walkdir::WalkDir;

// mod analyzer;
mod engine;
// mod graphviz;
// mod html;
// mod lsp;
mod vis_js;
mod models;
mod test_engine;
mod test_parser;
// mod tests;

fn main() -> Result<()> {
    // Placeholder main function to satisfy the compiler.
    Ok(())
}

// /// The primary entry point for the Tect toolset.
// #[derive(ClapParser, Debug)]
// #[command(author, version, about, long_about = None)]
// struct Args {
//     /// The architectural source file or directory to analyze.
//     input: Option<PathBuf>,

//     /// Output path:
//     /// - `.json` → raw semantic graph
//     /// - `.dot`  → Graphviz
//     /// - `.html` → Interactive visualization
//     #[arg(short, long)]
//     output: Option<PathBuf>,
// }

// #[tokio::main]
// async fn main() -> Result<()> {
//     let args_res = Args::try_parse();

//     if let Ok(args) = args_res {
//         if let Some(input_path) = args.input {
//             let mut analyzer = analyzer::TectAnalyzer::new();

//             let files = if input_path.is_dir() {
//                 WalkDir::new(input_path)
//                     .into_iter()
//                     .filter_map(|e| e.ok())
//                     .filter(|e| e.path().extension().is_some_and(|ext| ext == "tect"))
//                     .map(|e| e.path().to_path_buf())
//                     .collect::<Vec<_>>()
//             } else {
//                 vec![input_path]
//             };

//             for file in files {
//                 let content = fs::read_to_string(&file)?;
//                 let _ = analyzer.analyze(&content);
//             }

//             if let Some(out) = args.output {
//                 match out.extension().and_then(|e| e.to_str()) {
//                     Some("dot") => {
//                         let dot = graphviz::to_dot(&analyzer.graph);
//                         fs::write(out, dot)?;
//                     }
//                     Some("html") => {
//                         let dot = graphviz::to_dot(&analyzer.graph);
//                         let html = html::wrap_dot(&dot);
//                         fs::write(out, html)?;
//                     }
//                     _ => {
//                         let json = serde_json::to_string_pretty(&analyzer.graph)?;
//                         fs::write(out, json)?;
//                     }
//                 }
//             } else {
//                 let json = serde_json::to_string_pretty(&analyzer.graph)?;
//                 println!("{}", json);
//             }

//             return Ok(());
//         }
//     }

//     // Default: Language Server mode
//     let (service, socket) = LspService::new(|client| lsp::Backend {
//         client,
//         document_map: DashMap::new(),
//     });

//     Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
//         .serve(service)
//         .await;

//     Ok(())
// }
