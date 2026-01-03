#[cfg(test)]
mod tests {
    use crate::engine::Flow;
    use crate::models::*;
    use serde::Serialize;
    use std::fs::File;
    use std::io::Write;
    use std::sync::Arc;

    #[derive(Serialize)]
    struct GraphExport {
        nodes: Vec<Arc<Node>>,
        edges: Vec<Edge>,
    }

    #[test]
    fn generate_blog_architecture_json() -> std::io::Result<()> {
        // Define types (constants, variables, errors)
        let initial_command = Arc::new(Variable {
            name: "InitialCommand".to_string(),
            documentation: Some("The initial command input from the CLI".to_string()),
        });
        let path_to_config = Arc::new(Variable {
            name: "PathToConfig".to_string(),
            documentation: Some("The path to the configuration file".to_string()),
        });

        let settings = Arc::new(Constant {
            name: "Settings".to_string(),
            documentation: Some("The loaded settings from the config file".to_string()),
        });

        let templates = Arc::new(Constant {
            name: "Templates".to_string(),
            documentation: Some("The registry of HTML templates used for rendering".to_string()),
        });

        let source_file = Arc::new(Constant {
            name: "SourceFile".to_string(),
            documentation: Some("A raw input file found in the source directory".to_string()),
        });

        let article = Arc::new(Constant {
            name: "Article".to_string(),
            documentation: Some(
                "The processed data structure containing markdown content and metadata".to_string(),
            ),
        });

        let html_article = Arc::new(Variable {
            name: "HTMLarticle".to_string(),
            documentation: Some(
                "The final article HTML string ready to be written to disk".to_string(),
            ),
        });

        let html_index = Arc::new(Variable {
            name: "HTMLIndex".to_string(),
            documentation: Some(
                "The final index HTML string ready to be written to disk".to_string(),
            ),
        });

        let fs_error = Arc::new(Error {
            name: "FileSystemError".to_string(),
            documentation: Some(
                "Triggered when a file cannot be read from or written to the disk".to_string(),
            ),
        });

        let success = Arc::new(Variable {
            name: "SuccessReport".to_string(),
            documentation: Some(
                "A final summary of the operations performed during the run".to_string(),
            ),
        });

        // define functions

        let process_cli = Arc::new(Function {
            name: "ProcessCLI".to_string(),
            documentation: Some("Processes command-line input".to_string()),
            consumes: vec![Token::new(
                Arc::new(Kind::Variable(initial_command.clone())),
                Cardinality::Unitary,
                None,
            )],
            produces: vec![
                vec![Token::new(
                    Arc::new(Kind::Constant(settings.clone())),
                    Cardinality::Unitary,
                    None,
                )],
                vec![Token::new(
                    Arc::new(Kind::Variable(path_to_config.clone())),
                    Cardinality::Unitary,
                    None,
                )],
            ],
        });

        let load_config = Arc::new(Function {
            name: "LoadConfig".to_string(),
            documentation: Some("Loads configuration from a file".to_string()),
            consumes: vec![Token::new(
                Arc::new(Kind::Variable(path_to_config.clone())),
                Cardinality::Unitary,
                None,
            )],
            produces: vec![vec![Token::new(
                Arc::new(Kind::Constant(settings.clone())),
                Cardinality::Unitary,
                None,
            )]],
        });
        let load_templates = Arc::new(Function {
            name: "LoadTemplates".to_string(),
            documentation: Some("Loads HTML templates based on settings".to_string()),
            consumes: vec![Token::new(
                Arc::new(Kind::Constant(settings.clone())),
                Cardinality::Unitary,
                None,
            )],
            produces: vec![vec![Token::new(
                Arc::new(Kind::Constant(templates.clone())),
                Cardinality::Unitary,
                None,
            )]],
        });
        let scan_fs = Arc::new(Function {
            name: "ScanFS".to_string(),
            documentation: Some("Scans the filesystem for source files".to_string()),
            consumes: vec![Token::new(
                Arc::new(Kind::Constant(settings.clone())),
                Cardinality::Unitary,
                None,
            )],
            produces: vec![
                vec![Token::new(
                    Arc::new(Kind::Constant(source_file.clone())),
                    Cardinality::Collection,
                    None,
                )],
                vec![Token::new(
                    Arc::new(Kind::Error(fs_error.clone())),
                    Cardinality::Collection,
                    None,
                )],
            ],
        });

        let parse_markdown = Arc::new(Function {
            name: "ParseMarkdown".to_string(),
            documentation: Some("Parses markdown files into article structures".to_string()),
            consumes: vec![Token::new(
                Arc::new(Kind::Constant(source_file.clone())),
                Cardinality::Unitary,
                None,
            )],
            produces: vec![
                vec![Token::new(
                    Arc::new(Kind::Constant(article.clone())),
                    Cardinality::Unitary,
                    None,
                )],
                vec![Token::new(
                    Arc::new(Kind::Error(fs_error.clone())),
                    Cardinality::Unitary,
                    None,
                )],
            ],
        });
        let render_html_index = Arc::new(Function {
            name: "RenderHTMLIndex".to_string(),
            documentation: Some("Renders the index page into HTML using templates".to_string()),
            consumes: vec![
                Token::new(
                    Arc::new(Kind::Constant(article.clone())),
                    Cardinality::Collection,
                    None,
                ),
                Token::new(
                    Arc::new(Kind::Constant(settings.clone())),
                    Cardinality::Unitary,
                    None,
                ),
            ],
            produces: vec![vec![Token::new(
                Arc::new(Kind::Variable(html_index.clone())),
                Cardinality::Unitary,
                None,
            )]],
        });
        let render_html_articles = Arc::new(Function {
            name: "RenderHTMLArticles".to_string(),
            documentation: Some("Renders articles into HTML using templates".to_string()),
            consumes: vec![
                Token::new(
                    Arc::new(Kind::Constant(article.clone())),
                    Cardinality::Unitary,
                    None,
                ),
                Token::new(
                    Arc::new(Kind::Constant(templates.clone())),
                    Cardinality::Unitary,
                    None,
                ),
                Token::new(
                    Arc::new(Kind::Constant(settings.clone())),
                    Cardinality::Unitary,
                    None,
                ),
            ],
            produces: vec![vec![Token::new(
                Arc::new(Kind::Variable(html_article.clone())),
                Cardinality::Unitary,
                None,
            )]],
        });
        let write_index_to_disk = Arc::new(Function {
            name: "WriteIndexToDisk".to_string(),
            documentation: Some("Writes the index HTML file to disk".to_string()),
            consumes: vec![Token::new(
                Arc::new(Kind::Variable(html_index.clone())),
                Cardinality::Unitary,
                None,
            )],
            produces: vec![
                vec![Token::new(
                    Arc::new(Kind::Variable(success.clone())),
                    Cardinality::Unitary,
                    None,
                )],
                vec![Token::new(
                    Arc::new(Kind::Error(fs_error.clone())),
                    Cardinality::Unitary,
                    None,
                )],
            ],
        });
        let write_articles_to_disk = Arc::new(Function {
            name: "WriteArticlesToDisk".to_string(),
            documentation: Some("Writes HTML files to disk".to_string()),
            consumes: vec![Token::new(
                Arc::new(Kind::Variable(html_article.clone())),
                Cardinality::Unitary,
                None,
            )],
            produces: vec![
                vec![Token::new(
                    Arc::new(Kind::Variable(success.clone())),
                    Cardinality::Unitary,
                    None,
                )],
                vec![Token::new(
                    Arc::new(Kind::Error(fs_error.clone())),
                    Cardinality::Unitary,
                    None,
                )],
            ],
        });

        let functions = vec![
            process_cli,
            load_config,
            load_templates,
            scan_fs,
            parse_markdown,
            render_html_articles,
            render_html_index,
            write_articles_to_disk,
            write_index_to_disk,
        ];

        let mut flow = Flow::new();
        let (nodes, mut edges) = flow.process_flow(&functions);

        // Removes duplicated edges caused by the multiple pools
        // 1. Sort by logical identity: Source Name + Target Name + Kind
        edges.sort_unstable_by(|a, b| {
            a.source
                .cmp(&b.source)
                .then_with(|| a.target.cmp(&b.target))
                .then_with(|| format!("{:?}", a.token.kind).cmp(&format!("{:?}", b.token.kind)))
        });

        // 2. Deduplicate based on logical identity, ignoring the UID
        edges.dedup_by(|a, b| {
            a.source == b.source && a.target == b.target && a.token.kind == b.token.kind
        });

        // Serialization with 4-space indentation
        let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
        let mut buf = Vec::new();
        let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter.clone());

        GraphExport { nodes, edges }.serialize(&mut ser).unwrap();
        let json_data = String::from_utf8(buf).unwrap();

        let mut file = File::create("../experiments/architecture.json")?;
        file.write_all(json_data.as_bytes())?;

        // --- EXPORT 2: The Source Logic (List of Functions) ---
        let mut func_buf = Vec::new();
        let mut func_ser = serde_json::Serializer::with_formatter(&mut func_buf, formatter);
        functions.serialize(&mut func_ser).unwrap();

        let mut func_file = File::create("../experiments/functions.json")?;
        func_file.write_all(&func_buf)?;

        Ok(())
    }
}
