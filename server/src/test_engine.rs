#[cfg(test)]
mod tests {
    use crate::engine::Flow;
    use crate::models::*;
    use crate::vis_js;
    use std::fs;
    use std::io::Write;
    use std::sync::Arc;

    #[test]
    fn generate_blog_architecture_json() -> std::io::Result<()> {
        let mut structure = ProgramStructure::default();

        // --- Step 1: Baseline Identical Artifact Definitions ---
        let g_env = Arc::new(Group::new("Environment".into(), None));
        let g_ing = Arc::new(Group::new("Ingestion".into(), None));
        let g_ren = Arc::new(Group::new("Rendering".into(), None));
        let g_io = Arc::new(Group::new("IO".into(), None));

        structure.groups.insert("Environment".into(), g_env.clone());
        structure.groups.insert("Ingestion".into(), g_ing.clone());
        structure.groups.insert("Rendering".into(), g_ren.clone());
        structure.groups.insert("IO".into(), g_io.clone());

        let initial_command = Arc::new(Variable::new(
            "InitialCommand".into(),
            Some("The initial command input from the CLI".into()),
        ));
        let path_to_config = Arc::new(Variable::new(
            "PathToConfig".into(),
            Some("The path to the configuration file".into()),
        ));
        let settings = Arc::new(Constant::new(
            "Settings".into(),
            Some("The loaded settings from the config file".into()),
        ));
        let templates = Arc::new(Constant::new(
            "Templates".into(),
            Some("The registry of HTML templates used for rendering".into()),
        ));
        let source_file = Arc::new(Constant::new(
            "SourceFile".into(),
            Some("A raw input file found in the source directory".into()),
        ));
        let article = Arc::new(Constant::new(
            "Article".into(),
            Some("The processed data structure containing markdown content and metadata".into()),
        ));
        let html_article = Arc::new(Variable::new(
            "HTMLarticle".into(),
            Some("The final article HTML string ready to be written to disk".into()),
        ));
        let html_index = Arc::new(Variable::new(
            "HTMLIndex".into(),
            Some("The final index HTML string ready to be written to disk".into()),
        ));
        let fs_error = Arc::new(Error::new(
            "FileSystemError".into(),
            Some("Triggered when a file cannot be read from or written to the disk".into()),
        ));
        let success = Arc::new(Variable::new(
            "SuccessReport".into(),
            Some("A final summary of the operations performed during the run".into()),
        ));

        structure.artifacts.insert(
            "InitialCommand".into(),
            Kind::Variable(initial_command.clone()),
        );
        structure.artifacts.insert(
            "PathToConfig".into(),
            Kind::Variable(path_to_config.clone()),
        );
        structure
            .artifacts
            .insert("Settings".into(), Kind::Constant(settings.clone()));
        structure
            .artifacts
            .insert("Templates".into(), Kind::Constant(templates.clone()));
        structure
            .artifacts
            .insert("SourceFile".into(), Kind::Constant(source_file.clone()));
        structure
            .artifacts
            .insert("Article".into(), Kind::Constant(article.clone()));
        structure
            .artifacts
            .insert("HTMLarticle".into(), Kind::Variable(html_article.clone()));
        structure
            .artifacts
            .insert("HTMLIndex".into(), Kind::Variable(html_index.clone()));
        structure
            .artifacts
            .insert("FileSystemError".into(), Kind::Error(fs_error.clone()));
        structure
            .artifacts
            .insert("SuccessReport".into(), Kind::Variable(success.clone()));

        // --- Step 2: Baseline Identical Function Contracts ---
        let f_process_cli = Arc::new(Function::new(
            "ProcessCLI".into(),
            Some("Processes command-line input".into()),
            vec![Token::new(
                Kind::Variable(initial_command),
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
            Some(g_env.clone()),
        ));

        let f_load_config = Arc::new(Function::new(
            "LoadConfig".into(),
            Some("Loads configuration from a file".into()),
            vec![Token::new(
                Kind::Variable(path_to_config),
                Cardinality::Unitary,
            )],
            vec![vec![Token::new(
                Kind::Constant(settings.clone()),
                Cardinality::Unitary,
            )]],
            Some(g_env),
        ));

        let f_load_templates = Arc::new(Function::new(
            "LoadTemplates".into(),
            Some("Loads HTML templates based on settings".into()),
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

        let f_scan_fs = Arc::new(Function::new(
            "ScanFS".into(),
            Some("Scans the filesystem for source files".into()),
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
            Some(g_ing),
        ));

        let f_parse_markdown = Arc::new(Function::new(
            "ParseMarkdown".into(),
            Some("Parses markdown files into article structures".into()),
            vec![Token::new(
                Kind::Constant(source_file),
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

        let f_render_articles = Arc::new(Function::new(
            "RenderHTMLArticles".into(),
            Some("Renders articles into HTML using templates".into()),
            vec![
                Token::new(Kind::Constant(article.clone()), Cardinality::Unitary),
                Token::new(Kind::Constant(templates), Cardinality::Unitary),
                Token::new(Kind::Constant(settings.clone()), Cardinality::Unitary),
            ],
            vec![vec![Token::new(
                Kind::Variable(html_article.clone()),
                Cardinality::Unitary,
            )]],
            Some(g_ren.clone()),
        ));

        let f_render_index = Arc::new(Function::new(
            "RenderHTMLIndex".into(),
            Some("Renders the index page into HTML using templates".into()),
            vec![
                Token::new(Kind::Constant(article), Cardinality::Collection),
                Token::new(Kind::Constant(settings), Cardinality::Unitary),
            ],
            vec![vec![Token::new(
                Kind::Variable(html_index.clone()),
                Cardinality::Unitary,
            )]],
            Some(g_ren),
        ));

        let f_write_articles = Arc::new(Function::new(
            "WriteArticlesToDisk".into(),
            Some("Writes HTML files to disk".into()),
            vec![Token::new(
                Kind::Variable(html_article),
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
            Some(g_io.clone()),
        ));

        let f_write_index = Arc::new(Function::new(
            "WriteIndexToDisk".into(),
            Some("Writes the index HTML file to disk".into()),
            vec![Token::new(Kind::Variable(html_index), Cardinality::Unitary)],
            vec![
                vec![Token::new(Kind::Variable(success), Cardinality::Unitary)],
                vec![Token::new(Kind::Error(fs_error), Cardinality::Unitary)],
            ],
            Some(g_io),
        ));

        // Add to Catalog
        structure
            .catalog
            .insert("ProcessCLI".into(), f_process_cli.clone());
        structure
            .catalog
            .insert("LoadConfig".into(), f_load_config.clone());
        structure
            .catalog
            .insert("LoadTemplates".into(), f_load_templates.clone());
        structure.catalog.insert("ScanFS".into(), f_scan_fs.clone());
        structure
            .catalog
            .insert("ParseMarkdown".into(), f_parse_markdown.clone());
        structure
            .catalog
            .insert("RenderHTMLArticles".into(), f_render_articles.clone());
        structure
            .catalog
            .insert("RenderHTMLIndex".into(), f_render_index.clone());
        structure
            .catalog
            .insert("WriteArticlesToDisk".into(), f_write_articles.clone());
        structure
            .catalog
            .insert("WriteIndexToDisk".into(), f_write_index.clone());

        // Define Flow Sequence
        structure.flow = vec![
            "ProcessCLI".into(),
            "LoadConfig".into(),
            "LoadTemplates".into(),
            "ScanFS".into(),
            "ParseMarkdown".into(),
            "RenderHTMLArticles".into(),
            "RenderHTMLIndex".into(),
            "WriteArticlesToDisk".into(),
            "WriteIndexToDisk".into(),
        ];

        // --- Step 3: Simulation and Baseline Formatting ---
        let mut flow = Flow::new(true);
        let graph = flow.simulate(&structure);

        let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");

        // Export architecture.json
        let mut arch_buf = Vec::new();
        let mut arch_ser = serde_json::Serializer::with_formatter(&mut arch_buf, formatter.clone());
        serde::Serialize::serialize(&graph, &mut arch_ser).unwrap();
        fs::write("../experiments/architecture.json", arch_buf)?;

        // Export functions.json (Baseline Restoration)
        let functions = vec![
            f_process_cli,
            f_load_config,
            f_load_templates,
            f_scan_fs,
            f_parse_markdown,
            f_render_articles,
            f_render_index,
            f_write_articles,
            f_write_index,
        ];
        let mut func_buf = Vec::new();
        let mut func_ser = serde_json::Serializer::with_formatter(&mut func_buf, formatter);
        serde::Serialize::serialize(&functions, &mut func_ser).unwrap();
        fs::write("../experiments/functions.json", func_buf)?;

        // Export architecture.html
        fs::write(
            "../experiments/architecture.html",
            vis_js::generate_interactive_html(&graph),
        )?;

        Ok(())
    }
}
