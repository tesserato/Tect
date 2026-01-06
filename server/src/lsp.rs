//! # Tect Language Server Implementation
//!
//! Provides architectural intelligence including Tooltips (Hover),
//! Navigation (Go to Definition), and Context-Aware Canonical Formatting.
//! The formatting engine respects user-defined block separation by detecting
//! blank lines in the original source content.

use crate::analyzer::{Rule, TectAnalyzer, TectParser};
use crate::models::{Kind, ProgramStructure, Span};
use dashmap::DashMap;
use pest::Parser;
use regex::Regex;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

/// State container for the Tect Language Server.
pub struct Backend {
    #[allow(dead_code)]
    pub client: Client,
    /// Thread-safe map of document URLs to their source and analyzed metadata.
    pub document_state: DashMap<Url, (String, ProgramStructure)>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> LspResult<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
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

    /// Renders documentation tooltips for architectural symbols.
    async fn hover(&self, p: HoverParams) -> LspResult<Option<Hover>> {
        let uri = &p.text_document_position_params.text_document.uri;
        let pos = p.text_document_position_params.position;

        if let Some(state) = self.document_state.get(uri) {
            let (content, structure) = state.value();
            if let Some((word, range)) = Self::get_word_at(content, pos) {
                let markdown = if let Some(kind) = structure.artifacts.get(&word) {
                    let (lbl, docs) = match kind {
                        Kind::Constant(c) => ("Constant", &c.documentation),
                        Kind::Variable(v) => ("Variable", &v.documentation),
                        Kind::Error(e) => ("Error", &e.documentation),
                    };
                    format!(
                        "### {}: `{}`\n\n---\n\n{}",
                        lbl,
                        word,
                        docs.as_deref().unwrap_or("*No documentation.*")
                    )
                } else if let Some(f) = structure.catalog.get(&word) {
                    let group_line = f
                        .group
                        .as_ref()
                        .map(|g| format!("**Group**: `{}`\n\n", g.name))
                        .unwrap_or_default();
                    format!(
                        "### Function: `{}`\n\n{}---\n\n{}",
                        word,
                        group_line,
                        f.documentation.as_deref().unwrap_or("*No documentation.*")
                    )
                } else if let Some(g) = structure.groups.get(&word) {
                    format!(
                        "### Group: `{}`\n\n---\n\n{}",
                        word,
                        g.documentation
                            .as_deref()
                            .unwrap_or("*Architectural group.*")
                    )
                } else {
                    return Ok(None);
                };

                return Ok(Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: markdown,
                    }),
                    range: Some(range),
                }));
            }
        }
        Ok(None)
    }

    /// Jumps to the exact line and column of a symbol's declaration.
    async fn goto_definition(
        &self,
        p: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        let uri = &p.text_document_position_params.text_document.uri;
        let pos = p.text_document_position_params.position;

        if let Some(state) = self.document_state.get(uri) {
            let (content, structure) = state.value();
            if let Some((word, _)) = Self::get_word_at(content, pos) {
                let uid = if let Some(k) = structure.artifacts.get(&word) {
                    Some(k.uid())
                } else if let Some(f) = structure.catalog.get(&word) {
                    Some(f.uid)
                } else if let Some(g) = structure.groups.get(&word) {
                    Some(g.uid)
                } else {
                    None
                };

                if let Some(id) = uid {
                    if let Some(meta) = structure.symbol_table.get(&id) {
                        let range = Self::span_to_range(content, meta.definition_span);
                        return Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
                            uri.clone(),
                            range,
                        ))));
                    }
                }
            }
        }
        Ok(None)
    }

    /// Canonicalizes the Tect document format.
    /// Analyzes the gaps between tokens to preserve user-defined block separation
    /// while standardizing internal indentation and keyword spacing.
    async fn formatting(&self, p: DocumentFormattingParams) -> LspResult<Option<Vec<TextEdit>>> {
        let uri = &p.text_document.uri;
        if let Some(state) = self.document_state.get(uri) {
            let (content, _) = state.value();

            let mut entities = Vec::new();
            let mut current_block = String::new();
            let mut last_rule = None;
            let mut last_end_pos = 0;

            if let Ok(mut pairs) = TectParser::parse(Rule::program, content) {
                let program = pairs.next().unwrap();
                for pair in program.into_inner() {
                    let span = pair.as_span();
                    let rule = rule_to_logic(pair.as_rule());

                    // Detect if there was an empty line between the current and previous element.
                    // We check the gap in the original source content.
                    let gap = &content[last_end_pos..span.start()];
                    let has_empty_line_in_gap = gap.chars().filter(|&c| c == '\n').count() > 1;

                    // If a blank line was present, or the entity type changed, flush the current sequence.
                    if (has_empty_line_in_gap || last_rule != Some(rule))
                        && !current_block.is_empty()
                    {
                        entities.push(current_block.trim_end().to_string());
                        current_block = String::new();
                    }

                    match pair.as_rule() {
                        // Sequential entities: grouped together unless separated by a gap in the source.
                        Rule::comment | Rule::flow_step => {
                            current_block.push_str(pair.as_str().trim());
                            current_block.push_str("\n");
                        }

                        // Atomic Definitions: Always treated as discrete blocks.
                        Rule::const_def
                        | Rule::var_def
                        | Rule::err_def
                        | Rule::group_def
                        | Rule::func_def => {
                            let mut formatted = String::new();
                            if pair.as_rule() == Rule::func_def {
                                let mut inner = pair.clone().into_inner();
                                // Attached documentation lines
                                while let Some(p) = inner.peek() {
                                    if p.as_rule() == Rule::doc_line {
                                        formatted.push_str(inner.next().unwrap().as_str().trim());
                                        formatted.push_str("\n");
                                    } else {
                                        break;
                                    }
                                }
                                // Function Signature
                                let mut header_parts = Vec::new();
                                while let Some(p) = inner.peek() {
                                    if p.as_rule() == Rule::ident
                                        || p.as_rule() == Rule::kw_function
                                    {
                                        header_parts.push(inner.next().unwrap().as_str().trim());
                                    } else {
                                        break;
                                    }
                                }
                                formatted.push_str(&header_parts.join(" "));
                                // Inputs (parentheses-less)
                                if let Some(p) = inner.peek() {
                                    if p.as_rule() == Rule::token_list {
                                        formatted.push_str(" ");
                                        formatted.push_str(inner.next().unwrap().as_str().trim());
                                    }
                                }
                                formatted.push_str("\n");
                                // Output Branches
                                if let Some(p) = inner.next() {
                                    for out in p.into_inner() {
                                        formatted.push_str("    ");
                                        formatted.push_str(out.as_str().trim());
                                        formatted.push_str("\n");
                                    }
                                }
                            } else {
                                formatted = pair.as_str().trim().to_string();
                            }
                            entities.push(formatted.trim_end().to_string());
                        }
                        _ => {}
                    }

                    last_rule = Some(rule);
                    last_end_pos = span.end();
                }
            }

            // Finalize trailing block
            if !current_block.is_empty() {
                entities.push(current_block.trim_end().to_string());
            }

            if !entities.is_empty() {
                let final_text = entities.join("\n\n") + "\n";
                let full_range = Range::new(Position::new(0, 0), Position::new(u32::MAX, u32::MAX));
                return Ok(Some(vec![TextEdit::new(full_range, final_text)]));
            }
        }
        Ok(None)
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }
}

/// Simple classification for formatting blocks.
#[derive(PartialEq, Clone, Copy)]
enum FormatterRule {
    Comment,
    Flow,
    Atomic,
}

/// Maps grammar rules to formatting categories.
fn rule_to_logic(r: Rule) -> FormatterRule {
    match r {
        Rule::comment => FormatterRule::Comment,
        Rule::flow_step => FormatterRule::Flow,
        _ => FormatterRule::Atomic,
    }
}

impl Backend {
    /// Re-analyzes the document on content changes.
    async fn process_change(&self, uri: Url, content: String) {
        let mut analyzer = TectAnalyzer::new();
        if let Ok(structure) = analyzer.analyze(&content) {
            self.document_state.insert(uri, (content, structure));
        } else {
            self.document_state
                .entry(uri)
                .and_modify(|(old_content, _)| *old_content = content);
        }
    }

    /// Maps a UTF-16 cursor position to an architectural identifier.
    fn get_word_at(content: &str, pos: Position) -> Option<(String, Range)> {
        let line_str = content.lines().nth(pos.line as usize)?;

        let mut utf16_offset = 0;
        let mut byte_offset = 0;
        for c in line_str.chars() {
            if utf16_offset >= pos.character as usize {
                break;
            }
            utf16_offset += c.len_utf16();
            byte_offset += c.len_utf8();
        }

        let re = Regex::new(r"([a-zA-Z0-9_]+)").unwrap();
        for cap in re.find_iter(line_str) {
            if byte_offset >= cap.start() && byte_offset <= cap.end() {
                let start_utf16 = line_str[..cap.start()].encode_utf16().count() as u32;
                let end_utf16 = line_str[..cap.end()].encode_utf16().count() as u32;
                return Some((
                    cap.as_str().to_string(),
                    Range::new(
                        Position::new(pos.line, start_utf16),
                        Position::new(pos.line, end_utf16),
                    ),
                ));
            }
        }
        None
    }

    /// Maps a byte-offset Span to an LSP line/column range.
    fn span_to_range(content: &str, span: Span) -> Range {
        let mut start_pos = Position::new(0, 0);
        let mut end_pos = Position::new(0, 0);
        let mut cur_byte = 0;
        let mut line = 0;
        let mut col_utf16 = 0;

        for c in content.chars() {
            if cur_byte == span.start {
                start_pos = Position::new(line, col_utf16);
            }
            if cur_byte == span.end {
                end_pos = Position::new(line, col_utf16);
                break;
            }

            if c == '\n' {
                line += 1;
                col_utf16 = 0;
            } else {
                col_utf16 += c.len_utf16() as u32;
            }
            cur_byte += c.len_utf8();
        }
        Range::new(start_pos, end_pos)
    }
}
