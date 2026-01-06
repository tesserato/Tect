//! # Tect Language Server Backend
//!
//! Orchestrates the background analysis and LSP responses.
//! Implements "Last Known Good" state persistence for fluid editing.

use crate::analyzer::TectAnalyzer;
use crate::models::{Kind, ProgramStructure};
use dashmap::DashMap;
use regex::Regex;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

/// The implementation of the Tect Language Server backend.
pub struct Backend {
    #[allow(dead_code)]
    pub client: Client,
    /// Persistent storage for document state: (CurrentBuffer, LastSuccessfulBlueprint).
    pub document_state: DashMap<Url, (String, ProgramStructure)>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    /// Negotiates capabilities with the VS Code client upon connection.
    async fn initialize(&self, _: InitializeParams) -> LspResult<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn did_open(&self, p: DidOpenTextDocumentParams) {
        self.process_change(p.text_document.uri, p.text_document.text)
            .await;
    }

    async fn did_change(&self, p: DidChangeTextDocumentParams) {
        if let Some(c) = p.content_changes.into_iter().next() {
            self.process_change(p.text_document.uri, c.text).await;
        }
    }

    /// Provides architectural context for symbols under the cursor.
    ///
    /// Uses the Last Known Good (LKG) ProgramStructure to provide tooltips
    /// even when the user is mid-sentence or the file has syntax errors.
    async fn hover(&self, p: HoverParams) -> LspResult<Option<Hover>> {
        let uri = &p.text_document_position_params.text_document.uri;
        let pos = p.text_document_position_params.position;

        // 1. Retrieve the Last Known Good state for this document
        let Some(state) = self.document_state.get(uri) else {
            return Ok(None);
        };
        let (content, structure) = state.value();

        // 2. Extract the word under the cursor using a regex fallback
        let lines: Vec<&str> = content.lines().collect();
        let Some(line) = lines.get(pos.line as usize) else {
            return Ok(None);
        };

        let word_re = Regex::new(r"([a-zA-Z0-9_]+)").unwrap();
        for cap in word_re.find_iter(line) {
            if pos.character >= cap.start() as u32 && pos.character <= cap.end() as u32 {
                let word = cap.as_str();

                // 3. Resolve symbol against the Architectural IR
                let markdown_value = if let Some(kind) = structure.artifacts.get(word) {
                    // Logic for Constant, Variable, or Error artifacts
                    let (type_label, docs) = match kind {
                        Kind::Constant(c) => ("Constant", &c.documentation),
                        Kind::Variable(v) => ("Variable", &v.documentation),
                        Kind::Error(e) => ("Error", &e.documentation),
                    };

                    format!(
                        "### {}: `{}`\n\n--- \n\n{}",
                        type_label,
                        word,
                        docs.as_deref().unwrap_or("*No documentation provided.*")
                    )
                } else if let Some(func) = structure.catalog.get(word) {
                    // Logic for Function transformations
                    let group_line = func
                        .group
                        .as_ref()
                        .map(|g| format!("**Group**: `{}`\n\n", g.name))
                        .unwrap_or_default();

                    format!(
                        "### Function: `{}`\n\n{}--- \n\n{}",
                        word,
                        group_line,
                        func.documentation
                            .as_deref()
                            .unwrap_or("*No documentation provided.*")
                    )
                } else if let Some(group) = structure.groups.get(word) {
                    // Logic for Architectural Groups
                    format!(
                        "### Group: `{}`\n\n--- \n\n{}",
                        word,
                        group
                            .documentation
                            .as_deref()
                            .unwrap_or("*Logical architectural container.*")
                    )
                } else {
                    // Fallback for keywords or unresolved symbols
                    match word {
                        "constant" => "### Keyword: `constant`\nDefines a persistent data artifact.".into(),
                        "variable" => "### Keyword: `variable`\nDefines a linear, mutable data artifact.".into(),
                        "error" => "### Keyword: `error`\nDefines an architectural failure state.".into(),
                        "function" => "### Keyword: `function`\nDefines a transformation contract.".into(),
                        "group" => "### Keyword: `group`\nLogical architectural container for modular organization.".into(),
                        _ => format!("### Symbol: `{}`", word),
                    }
                };

                return Ok(Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: markdown_value,
                    }),
                    range: Some(Range::new(
                        Position::new(pos.line, cap.start() as u32),
                        Position::new(pos.line, cap.end() as u32),
                    )),
                }));
            }
        }

        Ok(None)
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }
}

impl Backend {
    /// Incremental analysis handler.
    /// Updates the architectural blueprint only on successful parsing (LKG strategy).
    async fn process_change(&self, uri: Url, content: String) {
        let mut analyzer = TectAnalyzer::new();
        match analyzer.analyze(&content) {
            Ok(structure) => {
                // Perfect parse: update the blueprint for high-fidelity tooltips
                self.document_state.insert(uri, (content, structure));
            }
            Err(_) => {
                // Syntax error: keep the previous structure so tooltips don't "flicker"
                self.document_state
                    .entry(uri)
                    .and_modify(|(old_content, _)| *old_content = content);
            }
        }
    }
}
