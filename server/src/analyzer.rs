//! # Tect Semantic Analyzer
//!
//! Responsible for parsing, dependency resolution, and semantic analysis.
//! Uses a SourceManager to handle multi-file projects and builds a global
//! ProgramStructure.

use crate::models::*;
use crate::source_manager::SourceManager;
use pest::iterators::{Pair, Pairs};
use pest::Parser;
use pest_derive::Parser;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tower_lsp::lsp_types::{DiagnosticSeverity, DiagnosticTag, Url};

#[derive(Parser)]
#[grammar = "tect.pest"]
pub struct TectParser;

/// The orchestrator for the analysis pipeline.
///
/// `Workspace` manages the state of the compiler service, including:
/// - The virtual file system (`SourceManager`).
/// - The global program structure (symbol table, flow graph, etc.).
/// - Context tracking for the current analysis session.
pub struct Workspace {
    /// Manages file contents and ID mapping.
    pub source_manager: SourceManager,
    /// The resulting Intermediate Representation (IR) after analysis.
    pub structure: ProgramStructure,
    /// Tracks the URI used as the entry point for the current analysis session.
    /// This is used to detect context switches (e.g., when the user switches tabs).
    pub current_root: Option<Url>,
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

impl Workspace {
    /// Creates a new, empty workspace.
    pub fn new() -> Self {
        Self {
            source_manager: SourceManager::new(),
            structure: ProgramStructure::default(),
            current_root: None,
        }
    }

    /// Entry point: Analyzes the project starting from a root URI.
    ///
    /// This method performs a full semantic analysis of the project. It follows these steps:
    /// 1. **Dependency Discovery**: Recursively scans imports starting from the root file to build the dependency graph.
    /// 2. **Cycle Detection**: Checks for circular dependencies in the graph.
    /// 3. **Multi-Pass Parsing**:
    ///     - **Pass 1 (Definitions)**: Parses all files to populate the symbol table (constants, variables, functions).
    ///     - **Pass 2 (Resolution)**: Parses files again to link function contracts and flow steps to defined symbols.
    /// 4. **Validation**: Checks for unused symbols and other global consistency rules.
    ///
    /// The `root_content` argument provides the current in-memory content of the root file (e.g., unsaved changes).
    pub fn analyze(&mut self, root_uri: Url, root_content: Option<String>) {
        self.current_root = Some(root_uri.clone());
        self.structure = ProgramStructure::default();

        // 1. Dependency Discovery
        let root_id = self.source_manager.get_id(&root_uri);
        self.source_manager.load_file(root_id, root_content);

        let mut parse_queue = vec![root_id];
        let mut visited_set = HashSet::new();
        let mut visited_order = Vec::new();
        let mut dependency_graph: HashMap<FileId, Vec<FileId>> = HashMap::new();

        // BFS to load and discover files
        let mut head = 0;
        while head < parse_queue.len() {
            let current_id = parse_queue[head];
            head += 1;

            if visited_set.contains(&current_id) {
                continue;
            }
            visited_set.insert(current_id);
            visited_order.push(current_id);

            // Ensure loaded
            if self.source_manager.get_content(current_id).is_none()
                && !self.source_manager.load_file(current_id, None)
            {
                self.report_error(
                    current_id,
                    None,
                    format!(
                        "Failed to read file: {:?}",
                        self.source_manager.get_uri(current_id)
                    ),
                );
                continue;
            }

            // Quick parse for imports to build graph
            let content_owned = self
                .source_manager
                .get_content(current_id)
                .map(|s| s.to_string());

            if let Some(content) = content_owned {
                let imports = self.scan_imports(&content, current_id);
                for (uri, _span) in imports {
                    let imported_id = self.source_manager.get_id(&uri);
                    dependency_graph
                        .entry(current_id)
                        .or_default()
                        .push(imported_id);

                    if !visited_set.contains(&imported_id) && !parse_queue.contains(&imported_id) {
                        parse_queue.push(imported_id);
                    }
                }
            }
        }

        // 2. Cycle Detection
        if let Some(cycle_path) = self.detect_cycle(root_id, &dependency_graph) {
            self.report_error(
                root_id,
                None,
                format!("Circular dependency detected: {}", cycle_path),
            );
            return;
        }

        // 3. Multi-Pass Parsing
        // Pass 1: Definitions
        for file_id in &visited_order {
            self.pass_definitions(*file_id);
        }

        // Pass 2: Resolution & Linking
        for file_id in &visited_order {
            self.pass_resolution(*file_id);
        }

        // 4. Validation (Unused Symbols)
        self.check_unused_symbols();
    }

    /// Scans a file for import statements to build the dependency graph.
    ///
    /// This does a shallow parse of the file to find `import` statements.
    /// It resolves relative paths (e.g., `./utils.tect`) against the file's URI
    /// and ensures the target file exists either in memory or on disk.
    fn scan_imports(&mut self, content: &str, file_id: FileId) -> Vec<(Url, Span)> {
        let mut results = Vec::new();
        if let Ok(mut pairs) = TectParser::parse(Rule::program, content) {
            if let Some(root) = pairs.next() {
                for pair in root.into_inner() {
                    if let Rule::import_stmt = pair.as_rule() {
                        let span = self.map_span(&pair, file_id);
                        let mut inner = pair.into_inner();
                        let _ = inner.next(); // import kw
                        let str_lit = inner.next().unwrap().as_str();
                        let rel_path = &str_lit[1..str_lit.len() - 1]; // strip quotes

                        // Clone base_uri to avoid conflicting borrow
                        let base_uri = self.source_manager.get_uri(file_id).cloned();

                        if let Some(base_uri) = base_uri {
                            // Use Url::join to handle relative paths (./, ../) correctly
                            if let Ok(target_uri) = base_uri.join(rel_path) {
                                let target_id = self.source_manager.get_id(&target_uri);
                                let is_in_memory =
                                    self.source_manager.get_content(target_id).is_some();
                                let exists_on_disk = target_uri
                                    .to_file_path()
                                    .map(|p| p.exists())
                                    .unwrap_or(false);

                                // Check memory first (unsaved files), then disk
                                if is_in_memory || exists_on_disk {
                                    results.push((target_uri, span));
                                } else {
                                    self.report_error(
                                        file_id,
                                        Some(span),
                                        format!("Import not found: '{}'", rel_path),
                                    );
                                }
                            } else {
                                self.report_error(
                                    file_id,
                                    Some(span),
                                    format!("Invalid import path: '{}'", rel_path),
                                );
                            }
                        }
                    }
                }
            }
        }
        results
    }

    /// Detects cycles in the dependency graph using Depth-First Search (DFS).
    ///
    /// Returns `Some(String)` containing the cycle path if one is detected, or `None` otherwise.
    fn detect_cycle(&self, root: FileId, graph: &HashMap<FileId, Vec<FileId>>) -> Option<String> {
        let mut visited = HashSet::new();
        let mut recursion_stack = HashSet::new();
        let mut path_stack = Vec::new();

        self.dfs_cycle(
            root,
            graph,
            &mut visited,
            &mut recursion_stack,
            &mut path_stack,
        )
    }

    fn dfs_cycle(
        &self,
        current: FileId,
        graph: &HashMap<FileId, Vec<FileId>>,
        visited: &mut HashSet<FileId>,
        recursion_stack: &mut HashSet<FileId>,
        path_stack: &mut Vec<FileId>,
    ) -> Option<String> {
        visited.insert(current);
        recursion_stack.insert(current);
        path_stack.push(current);

        if let Some(neighbors) = graph.get(&current) {
            for &neighbor in neighbors {
                if recursion_stack.contains(&neighbor) {
                    // Cycle found
                    let cycle_path: Vec<String> = path_stack
                        .iter()
                        .chain(std::iter::once(&neighbor))
                        .filter_map(|id| self.source_manager.get_uri(*id).map(|u| u.to_string()))
                        .collect();
                    return Some(cycle_path.join(" -> "));
                }
                if !visited.contains(&neighbor) {
                    if let Some(cycle) =
                        self.dfs_cycle(neighbor, graph, visited, recursion_stack, path_stack)
                    {
                        return Some(cycle);
                    }
                }
            }
        }

        recursion_stack.remove(&current);
        path_stack.pop();
        None
    }

    // --- Pass 1: Definitions ---

    /// Pass 1 traverses the file to register all top-level definitions.
    ///
    /// This includes:
    /// - Constants, Variables, Errors (Artifacts)
    /// - Groups
    /// - Function skeletons (name, group, docs), but NOT their contracts.
    ///
    /// This ensures that symbols are available for resolution in Pass 2, regardless of definition order.
    fn pass_definitions(&mut self, file_id: FileId) {
        let content: &str = match self.source_manager.get_content(file_id) {
            Some(c) => c,
            None => return,
        };
        let content_owned = content.to_string();

        let parse_res = TectParser::parse(Rule::program, &content_owned);
        let pairs = match parse_res {
            Ok(mut p) => p.next().unwrap(),
            Err(e) => {
                let start_off = match e.line_col {
                    pest::error::LineColLocation::Pos((l, c)) => {
                        self.pos_to_offset(&content_owned, l, c)
                    }
                    pest::error::LineColLocation::Span((l, c), _) => {
                        self.pos_to_offset(&content_owned, l, c)
                    }
                };
                let end_off = start_off;
                self.report_error(
                    file_id,
                    Some(Span::new(file_id, start_off, end_off)),
                    format!("Syntax Error: {}", e.variant.message()),
                );
                return;
            }
        };

        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::const_def => self.define_type(&pair, "constant", file_id),
                Rule::var_def => self.define_type(&pair, "variable", file_id),
                Rule::err_def => self.define_type(&pair, "error", file_id),
                Rule::group_def => self.define_group(&pair, file_id),
                Rule::func_def => self.define_function_skeleton(&pair, file_id),
                _ => {}
            }
        }
    }

    // --- Pass 2: Resolution ---

    /// Pass 2 resolves symbol references and constructs the full logical model.
    ///
    /// This includes:
    /// - Parsing function signatures (inputs/outputs) and linking them to defined artifacts.
    /// - Building the flow sequence and linking flow steps to functions.
    /// - Validating that all referenced symbols exist.
    fn pass_resolution(&mut self, file_id: FileId) {
        let content: &str = match self.source_manager.get_content(file_id) {
            Some(c) => c,
            None => return,
        };
        let content_owned = content.to_string();

        let parse_res = TectParser::parse(Rule::program, &content_owned);
        let pairs = match parse_res {
            Ok(mut p) => p.next().unwrap(),
            Err(_) => return, // Handled in Pass 1
        };

        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::func_def => self.link_function_contracts(&pair, file_id),
                Rule::flow_step => {
                    let name = pair.as_str().trim();
                    if !name.is_empty() {
                        let span = self.map_span(&pair, file_id);
                        self.structure.flow.push(FlowStep {
                            function_name: name.to_string(),
                            span,
                        });

                        if let Some(func) = self.structure.catalog.get(name) {
                            self.add_occurrence(func.uid, span);
                        } else {
                            self.report_error(
                                file_id,
                                Some(span),
                                format!("Undefined function: '{}'", name),
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn check_unused_symbols(&mut self) {
        for meta in self.structure.symbol_table.values() {
            if meta.occurrences.len() == 1 {
                self.structure.diagnostics.push(DiagnosticWithContext {
                    file_id: meta.definition_span.file_id,
                    span: Some(meta.definition_span),
                    message: format!("Unused symbol: '{}'", meta.name),
                    severity: DiagnosticSeverity::WARNING,
                    tags: vec![DiagnosticTag::UNNECESSARY],
                });
            }
        }
    }

    // --- Helpers ---

    fn map_span(&self, p: &Pair<Rule>, file_id: FileId) -> Span {
        let s = p.as_span();
        Span::new(file_id, s.start(), s.end())
    }

    fn add_occurrence(&mut self, uid: u32, span: Span) {
        if let Some(meta) = self.structure.symbol_table.get_mut(&uid) {
            meta.occurrences.push(span);
        }
    }

    fn check_duplicate(&mut self, name: &str, span: Span) -> bool {
        if self.structure.artifacts.contains_key(name)
            || self.structure.groups.contains_key(name)
            || self.structure.catalog.contains_key(name)
        {
            self.report_error(
                span.file_id,
                Some(span),
                format!("Symbol '{}' is already defined.", name),
            );
            return true;
        }
        false
    }

    fn report_error(&mut self, file_id: FileId, span: Option<Span>, msg: String) {
        self.structure.diagnostics.push(DiagnosticWithContext {
            file_id,
            span,
            message: msg,
            severity: DiagnosticSeverity::ERROR,
            tags: vec![],
        });
    }

    // --- Definition Logic ---

    fn define_type(&mut self, pair: &Pair<Rule>, kw: &str, file_id: FileId) {
        let mut inner = pair.clone().into_inner();
        let doc_str = self.collect_docs(&mut inner);
        let _kw = inner.next().unwrap();
        let name_p = inner.next().unwrap();
        let name = name_p.as_str().to_string();
        let span = self.map_span(&name_p, file_id);

        if self.check_duplicate(&name, span) {
            return;
        }

        let kind = match kw {
            "constant" => Kind::Constant(Arc::new(Constant::new(name.clone(), doc_str))),
            "variable" => Kind::Variable(Arc::new(Variable::new(name.clone(), doc_str))),
            _ => Kind::Error(Arc::new(Error::new(name.clone(), doc_str))),
        };

        self.structure.symbol_table.insert(
            kind.uid(),
            SymbolMetadata {
                name: name.clone(),
                definition_span: span,
                occurrences: vec![span],
            },
        );
        self.structure.artifacts.insert(name, kind);
    }

    fn define_group(&mut self, pair: &Pair<Rule>, file_id: FileId) {
        let mut inner = pair.clone().into_inner();
        let doc_str = self.collect_docs(&mut inner);
        let _kw = inner.next().unwrap();
        let name_p = inner.next().unwrap();
        let name = name_p.as_str().to_string();
        let span = self.map_span(&name_p, file_id);

        if self.check_duplicate(&name, span) {
            return;
        }

        let group = Arc::new(Group::new(name.clone(), doc_str));
        self.structure.symbol_table.insert(
            group.uid,
            SymbolMetadata {
                name: name.clone(),
                definition_span: span,
                occurrences: vec![span],
            },
        );
        self.structure.groups.insert(name, group);
    }

    fn define_function_skeleton(&mut self, pair: &Pair<Rule>, file_id: FileId) {
        let mut inner = pair.clone().into_inner();
        let doc_str = self.collect_docs(&mut inner);
        let mut group = None;

        if let Some(p) = inner.peek() {
            if p.as_rule() == Rule::ident {
                let g_name_p = inner.next().unwrap();
                let g_name = g_name_p.as_str();
                if let Some(g) = self.structure.groups.get(g_name).cloned() {
                    group = Some(g.clone());
                    self.add_occurrence(g.uid, self.map_span(&g_name_p, file_id));
                } else {
                    self.report_error(
                        file_id,
                        Some(self.map_span(&g_name_p, file_id)),
                        format!("Undefined group: '{}'", g_name),
                    );
                }
            }
        }

        let _kw = inner.next().unwrap();
        let name_p = inner.next().unwrap();
        let name = name_p.as_str().to_string();
        let span = self.map_span(&name_p, file_id);

        if self.check_duplicate(&name, span) {
            return;
        }

        let function = Arc::new(Function::new_skeleton(name.clone(), doc_str, group));
        self.structure.symbol_table.insert(
            function.uid,
            SymbolMetadata {
                name: name.clone(),
                definition_span: span,
                occurrences: vec![span],
            },
        );
        self.structure.catalog.insert(name, function);
    }

    fn link_function_contracts(&mut self, pair: &Pair<Rule>, file_id: FileId) {
        let mut inner = pair.clone().into_inner();
        // Skip docs and group prefix
        while let Some(p) = inner.peek() {
            if p.as_rule() == Rule::doc_line {
                inner.next();
            } else {
                break;
            }
        }
        if let Some(p) = inner.peek() {
            if p.as_rule() == Rule::ident {
                inner.next();
            }
        }
        let _kw = inner.next();
        let name_p = inner.next().unwrap();
        let name = name_p.as_str();

        // Stable context seed: Function name
        let func_ctx = name;

        let mut consumes = Vec::new();
        if let Some(p) = inner.peek() {
            if p.as_rule() == Rule::token_list {
                consumes = self.resolve_tokens(inner.next().unwrap(), file_id, func_ctx, "in");
            }
        }

        let mut produces = Vec::new();
        // Check if optional output block exists
        if let Some(outputs_pair) = inner.next() {
            if outputs_pair.as_rule() == Rule::func_outputs {
                for (i, line) in outputs_pair.into_inner().enumerate() {
                    let list = line.into_inner().next().unwrap();
                    produces.push(self.resolve_tokens(
                        list,
                        file_id,
                        func_ctx,
                        &format!("out_{}", i),
                    ));
                }
            }
        }

        if let Some(func) = self.structure.catalog.get_mut(name) {
            let f = Arc::get_mut(func).unwrap();
            f.consumes = consumes;
            f.produces = produces;
        }
    }

    /// Resolves a list of tokens string representations into concrete `Token` instances.
    ///
    /// This function:
    /// 1. LOOKS UP the artifact definition in the symbol table.
    /// 2. Creates a unique, deterministic UID for this specific usage of the token (context-dependent).
    /// 3. Validates that the artifact is defined.
    fn resolve_tokens(
        &mut self,
        pair: Pair<Rule>,
        file_id: FileId,
        ctx_func: &str,
        ctx_dir: &str,
    ) -> Vec<Token> {
        let mut tokens = Vec::new();
        for (i, t_pair) in pair.into_inner().enumerate() {
            let inner = t_pair.into_inner().next().unwrap();
            let (name, card, span) = match inner.as_rule() {
                Rule::collection => {
                    let ident_p = inner.into_inner().next().unwrap();
                    (
                        ident_p.as_str(),
                        Cardinality::Collection,
                        self.map_span(&ident_p, file_id),
                    )
                }
                _ => (
                    inner.as_str(),
                    Cardinality::Unitary,
                    self.map_span(&inner, file_id),
                ),
            };

            if let Some(kind) = self.structure.artifacts.get(name) {
                let k = kind.clone();
                self.add_occurrence(k.uid(), span);

                // Deterministic UID for this token usage
                // hash(FunctionName + Direction + Index + TypeName)
                let token_sig = format!("{}:{}:{}:{}", ctx_func, ctx_dir, i, name);
                let uid = hash_name(&token_sig);

                tokens.push(Token::new(k, card, uid));
            } else {
                self.report_error(
                    file_id,
                    Some(span),
                    format!(
                        "Undefined artifact: '{}'. All types must be defined before use.",
                        name
                    ),
                );
            }
        }
        tokens
    }

    fn collect_docs(&self, inner: &mut Pairs<Rule>) -> Option<String> {
        let mut docs = Vec::new();
        while let Some(p) = inner.peek() {
            if p.as_rule() == Rule::doc_line {
                let raw = inner.next().unwrap().as_str();
                docs.push(raw.trim_start_matches('#').trim().to_string());
            } else {
                break;
            }
        }
        if docs.is_empty() {
            None
        } else {
            Some(docs.join("\n"))
        }
    }

    fn pos_to_offset(&self, content: &str, line: usize, col: usize) -> usize {
        let mut curr_line = 1;
        let mut curr_col = 1;
        for (i, c) in content.char_indices() {
            if curr_line == line && curr_col == col {
                return i;
            }
            if c == '\n' {
                curr_line += 1;
                curr_col = 1;
            } else {
                curr_col += 1;
            }
        }
        content.len()
    }
}
