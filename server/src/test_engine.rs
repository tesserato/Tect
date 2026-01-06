#[cfg(test)]
mod tests {
    use crate::engine::Flow;
    use crate::models::*;
    use crate::vis_js;
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
        // Define Groups
        let g_env = Some(Arc::new(Group::new("Environment".to_string(), None)));
        let g_ing = Some(Arc::new(Group::new("Ingestion".to_string(), None)));
        let g_ren = Some(Arc::new(Group::new("Rendering".to_string(), None)));
        let g_io = Some(Arc::new(Group::new("IO".to_string(), None)));

        // Define types (constants, variables, errors)
        let initial_command = Arc::new(Variable::new(
            "InitialCommand".to_string(),
            Some("The initial command input from the CLI".to_string()),
        ));
        let path_to_config = Arc::new(Variable::new(
            "PathToConfig".to_string(),
            Some("The path to the configuration file".to_string()),
        ));

        let settings = Arc::new(Constant::new(
            "Settings".to_string(),
            Some("The loaded settings from the config file".to_string()),
        ));

        let templates = Arc::new(Constant::new(
            "Templates".to_string(),
            Some("The registry of HTML templates used for rendering".to_string()),
        ));

        let source_file = Arc::new(Constant::new(
            "SourceFile".to_string(),
            Some("A raw input file found in the source directory".to_string()),
        ));

        let article = Arc::new(Constant::new(
            "Article".to_string(),
            Some(
                "The processed data structure containing markdown content and metadata".to_string(),
            ),
        ));

        let html_article = Arc::new(Variable::new(
            "HTMLarticle".to_string(),
            Some("The final article HTML string ready to be written to disk".to_string()),
        ));

        let html_index = Arc::new(Variable::new(
            "HTMLIndex".to_string(),
            Some("The final index HTML string ready to be written to disk".to_string()),
        ));

        let fs_error = Arc::new(Error::new(
            "FileSystemError".to_string(),
            Some("Triggered when a file cannot be read from or written to the disk".to_string()),
        ));

        let success = Arc::new(Variable::new(
            "SuccessReport".to_string(),
            Some("A final summary of the operations performed during the run".to_string()),
        ));

        // define functions

        let process_cli = Arc::new(Function::new(
            "ProcessCLI".to_string(),
            Some("Processes command-line input".to_string()),
            vec![Token::new(
                Kind::Variable(initial_command.clone()),
                Cardinality::Unitary,
            )],
            vec![
                vec![Token::new(
                    Kind::Constant(settings.clone()),
                    Cardinality::Unitary,
                )],
                vec![Token::new(
                    Kind::Variable(path_to_config.clone()),
                    Cardinality::Unitary,
                )],
            ],
            g_env.clone(),
        ));

        let load_config = Arc::new(Function::new(
            "LoadConfig".to_string(),
            Some("Loads configuration from a file".to_string()),
            vec![Token::new(
                Kind::Variable(path_to_config.clone()),
                Cardinality::Unitary,
            )],
            vec![vec![Token::new(
                Kind::Constant(settings.clone()),
                Cardinality::Unitary,
            )]],
            g_env.clone(),
        ));
        let load_templates = Arc::new(Function::new(
            "LoadTemplates".to_string(),
            Some("Loads HTML templates based on settings".to_string()),
            vec![Token::new(
                Kind::Constant(settings.clone()),
                Cardinality::Unitary,
            )],
            vec![vec![Token::new(
                Kind::Constant(templates.clone()),
                Cardinality::Unitary,
            )]],
            None,
        ));
        let scan_fs = Arc::new(Function::new(
            "ScanFS".to_string(),
            Some("Scans the filesystem for source files".to_string()),
            vec![Token::new(
                Kind::Constant(settings.clone()),
                Cardinality::Unitary,
            )],
            vec![
                vec![Token::new(
                    Kind::Constant(source_file.clone()),
                    Cardinality::Collection,
                )],
                vec![Token::new(
                    Kind::Error(fs_error.clone()),
                    Cardinality::Collection,
                )],
            ],
            g_ing.clone(),
        ));

        let parse_markdown = Arc::new(Function::new(
            "ParseMarkdown".to_string(),
            Some("Parses markdown files into article structures".to_string()),
            vec![Token::new(
                Kind::Constant(source_file.clone()),
                Cardinality::Unitary,
            )],
            vec![
                vec![Token::new(
                    Kind::Constant(article.clone()),
                    Cardinality::Unitary,
                )],
                vec![Token::new(
                    Kind::Error(fs_error.clone()),
                    Cardinality::Unitary,
                )],
            ],
            None,
        ));
        let render_html_index = Arc::new(Function::new(
            "RenderHTMLIndex".to_string(),
            Some("Renders the index page into HTML using templates".to_string()),
            vec![
                Token::new(Kind::Constant(article.clone()), Cardinality::Collection),
                Token::new(Kind::Constant(settings.clone()), Cardinality::Unitary),
            ],
            vec![vec![Token::new(
                Kind::Variable(html_index.clone()),
                Cardinality::Unitary,
            )]],
            g_ren.clone(),
        ));
        let render_html_articles = Arc::new(Function::new(
            "RenderHTMLArticles".to_string(),
            Some("Renders articles into HTML using templates".to_string()),
            vec![
                Token::new(Kind::Constant(article.clone()), Cardinality::Unitary),
                Token::new(Kind::Constant(templates.clone()), Cardinality::Unitary),
                Token::new(Kind::Constant(settings.clone()), Cardinality::Unitary),
            ],
            vec![vec![Token::new(
                Kind::Variable(html_article.clone()),
                Cardinality::Unitary,
            )]],
            g_ren.clone(),
        ));
        let write_index_to_disk = Arc::new(Function::new(
            "WriteIndexToDisk".to_string(),
            Some("Writes the index HTML file to disk".to_string()),
            vec![Token::new(
                Kind::Variable(html_index.clone()),
                Cardinality::Unitary,
            )],
            vec![
                vec![Token::new(
                    Kind::Variable(success.clone()),
                    Cardinality::Unitary,
                )],
                vec![Token::new(
                    Kind::Error(fs_error.clone()),
                    Cardinality::Unitary,
                )],
            ],
            g_io.clone(),
        ));
        let write_articles_to_disk = Arc::new(Function::new(
            "WriteArticlesToDisk".to_string(),
            Some("Writes HTML files to disk".to_string()),
            vec![Token::new(
                Kind::Variable(html_article.clone()),
                Cardinality::Unitary,
            )],
            vec![
                vec![Token::new(
                    Kind::Variable(success.clone()),
                    Cardinality::Unitary,
                )],
                vec![Token::new(
                    Kind::Error(fs_error.clone()),
                    Cardinality::Unitary,
                )],
            ],
            g_io.clone(),
        ));

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

        let mut flow = Flow::new(true);
        let (nodes, edges) = flow.process_flow(&functions);

        // Serialization with 4-space indentation (Per User Requirement)
        let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
        let mut buf = Vec::new();
        let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter.clone());

        GraphExport {
            nodes: nodes.clone(),
            edges: edges.clone(),
        }
        .serialize(&mut ser)
        .unwrap();

        let json_data = String::from_utf8(buf).unwrap();

        let mut file = File::create("../experiments/architecture.json")?;
        file.write_all(json_data.as_bytes())?;

        // --- EXPORT 2: The Source Logic (List of Functions) ---
        let mut func_buf = Vec::new();
        let mut func_ser = serde_json::Serializer::with_formatter(&mut func_buf, formatter);
        functions.serialize(&mut func_ser).unwrap();

        let mut func_file = File::create("../experiments/functions.json")?;
        func_file.write_all(&func_buf)?;

        // --- EXPORT 3: Interactive HTML (Ported from plot.py) ---
        let graph = Graph { nodes, edges };
        let html_content = vis_js::generate_interactive_html(&graph);
        let mut html_file = File::create("../experiments/architecture.html")?;
        html_file.write_all(html_content.as_bytes())?;

        Ok(())
    }
}
