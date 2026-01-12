//! # Tect Logic Engine
//!
//! Orchestrates the architectural simulation by consuming a [ProgramStructure].
//! It tracks the movement of tokens through pools and handles branching.
//!
//! This module is decoupled from source text. It reports logical errors via
//! [DiagnosticWithContext] which the LSP layer later resolves to file ranges.

use crate::models::*;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tower_lsp::lsp_types::DiagnosticSeverity;

/// Result of a consumption attempt by a node.
pub enum Consumed {
    /// All required tokens were found and consumed. Returns the edges created.
    AllTokens(Vec<Edge>),
    /// Some tokens were missing. Returns the list of missing tokens.
    SomeTokens(Vec<Token>),
}

/// Represents tokens that were not consumed during the simulation step.
pub struct Leftovers {
    /// Unused variable tokens.
    pub variables: Vec<Token>,
    /// Unhandled error tokens.
    pub errors: Vec<Token>,
    /// Unused constant tokens.
    pub constants: Vec<Token>,
}

/// Evaluates the availability of tokens for a specific execution path.
///
/// A `TokenPool` represents the state of available data at a specific point in the
/// execution flow (similar to a scope or frame). It tracks:
/// - Available variables, errors, and constants.
/// - The origin node of each token (for backtracking edges).
/// - Which nodes have been expanded (visited).
#[derive(Clone)]
pub struct TokenPool {
    pub variables: Vec<Token>,
    pub errors: Vec<Token>,
    pub constants: Vec<Token>,
    pub token_to_origin_node: HashMap<u32, Arc<Node>>,
    pub expanded_nodes: HashSet<u32>,
    pub constants_used: HashSet<u32>,
}

impl TokenPool {
    /// Creates a new pool seeded with a list of "External Input" tokens.
    /// These are attributed to the `initial_node`.
    pub fn new(initial_requirements: Vec<Token>, initial_node: Arc<Node>) -> Self {
        let mut variables = Vec::new();
        let mut errors = Vec::new();
        let mut constants = Vec::new();
        let mut token_to_origin_node = HashMap::new();

        for token in initial_requirements {
            // Map the token UID to the artificial initial node.
            token_to_origin_node.insert(token.uid, initial_node.clone());
            match &token.kind {
                Kind::Variable(..) => variables.push(token),
                Kind::Error(..) => errors.push(token),
                Kind::Constant(..) => constants.push(token),
            };
        }
        Self {
            variables,
            errors,
            constants,
            token_to_origin_node,
            expanded_nodes: HashSet::new(),
            constants_used: HashSet::new(),
        }
    }

    /// Adds new tokens produced by a node to the pool.
    ///
    /// If the producer node was already expanded (visited), the tokens are marked
    /// with `Cardinality::Collection` to represent that they might be produced multiple times
    /// (e.g., in a loop).
    pub fn produce(&mut self, tokens: Vec<Token>, producer: Arc<Node>) {
        let is_expanded = self.expanded_nodes.contains(&producer.uid);
        for mut token in tokens {
            if is_expanded {
                token.cardinality = Cardinality::Collection;
            }
            self.token_to_origin_node
                .insert(token.uid, producer.clone());
            match &token.kind {
                Kind::Variable(..) => self.variables.push(token),
                Kind::Error(..) => self.errors.push(token),
                Kind::Constant(..) => self.constants.push(token),
            };
        }
    }

    /// attempts to fulfill the input requirements of a destination node from the pool.
    ///
    /// # Logic
    /// - Matches requirements against available tokens by Kind UID.
    /// - Checks for infinite loops/recursion (Unitary requirement satisfying Collection token).
    /// - Creates data flow edges for consumed tokens.
    ///
    /// # Returns
    /// - `Consumed::AllTokens` with edges if successful.
    /// - `Consumed::SomeTokens` with missing requirements if failed.
    pub fn try_to_consume(&mut self, requirements: Vec<Token>, destination: Arc<Node>) -> Consumed {
        let mut edges = Vec::new();
        let mut consumed_in_step: Vec<Token> = Vec::new();
        let mut trigger_expansion = false;

        for req in &requirements {
            let pool = match &req.kind {
                Kind::Variable(..) => &self.variables,
                Kind::Error(..) => &self.errors,
                Kind::Constant(..) => &self.constants,
            };

            // Match based on Kind (Artifact ID), ignoring the specific Token ID instance
            let matched = pool
                .iter()
                .find(|t| t.kind.uid() == req.kind.uid() && !consumed_in_step.contains(t))
                .cloned();

            if let Some(t) = matched {
                if let Some(origin) = self.token_to_origin_node.get(&t.uid) {
                    if req.cardinality == Cardinality::Unitary
                        && t.cardinality == Cardinality::Collection
                    {
                        trigger_expansion = true;
                    }
                    edges.push(Edge {
                        from_node_uid: origin.uid,
                        to_node_uid: destination.uid,
                        token: t.clone(),
                        relation: EdgeRelation::DataFlow,
                    });
                    consumed_in_step.push(t);
                }
            }
        }

        let missing: Vec<Token> = requirements
            .iter()
            .filter(|req| {
                !consumed_in_step
                    .iter()
                    .any(|c| c.kind.uid() == req.kind.uid())
            })
            .cloned()
            .collect();

        if missing.is_empty() {
            if trigger_expansion {
                self.expanded_nodes.insert(destination.uid);
            }
            for used in &consumed_in_step {
                if matches!(used.kind, Kind::Constant(..)) {
                    self.constants_used.insert(used.uid);
                }
            }
            self.variables.retain(|t| !consumed_in_step.contains(t));
            self.errors.retain(|t| !consumed_in_step.contains(t));
            Consumed::AllTokens(edges)
        } else {
            Consumed::SomeTokens(missing)
        }
    }

    /// Returns a snapshot of unused tokens.
    pub fn get_leftover_tokens(&self) -> Leftovers {
        Leftovers {
            variables: self.variables.clone(),
            errors: self.errors.clone(),
            constants: self
                .constants
                .iter()
                .filter(|c| !self.constants_used.contains(&c.uid))
                .cloned()
                .collect(),
        }
    }
}

/// Manages the full architectural flow simulation.
pub struct Flow {
    /// Ordered list of nodes derived from the simulation.
    pub nodes: Vec<Arc<Node>>,
    /// List of all data flow and control edges.
    pub edges: Vec<Edge>,
    /// Active token pools (representing parallel branches of execution).
    pub pools: Vec<TokenPool>,
    /// Whether to remove duplicate edges in the final graph.
    pub deduplicate_edges: bool,
    /// Diagnostics collected during simulation (e.g., flow errors).
    pub diagnostics: Vec<DiagnosticWithContext>,
}

impl Flow {
    pub fn new(deduplicate_edges: bool) -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            pools: Vec::new(),
            deduplicate_edges,
            diagnostics: Vec::new(),
        }
    }

    /// Simulates the flow based on the global program structure.
    pub fn simulate(&mut self, structure: &ProgramStructure) -> Graph {
        // Prepare artificial nodes (but do not add them to graph yet)
        let initial_node = Arc::new(Node::new_artificial(
            "InitialNode".to_string(),
            true,
            false,
            false,
        ));

        let final_node = Arc::new(Node::new_artificial(
            "FinalNode".to_string(),
            false,
            true,
            false,
        ));

        let fatal_node = Arc::new(Node::new_artificial(
            "FatalErrors".to_string(),
            false,
            false,
            true,
        ));

        // 1. Seed Initial Pool
        let mut initial_tokens = Vec::new();
        let mut seen_files = HashSet::new();

        for step in &structure.flow {
            if seen_files.insert(step.span.file_id) {
                if let Some(func) = structure.catalog.get(&step.function_name) {
                    initial_tokens.extend(func.consumes.clone());
                }
            }
        }

        self.pools
            .push(TokenPool::new(initial_tokens, initial_node.clone()));

        // 2. Simulation Loop
        for step in &structure.flow {
            let Some(func) = structure.catalog.get(&step.function_name) else {
                continue;
            };
            let node = Arc::new(Node::new(func.clone()));
            self.nodes.push(node.clone());

            let mut next_pools = Vec::new();
            let mut step_executed_at_least_once = false;
            let mut missing_tokens_examples = HashSet::new();

            for pool in &mut self.pools {
                match pool.try_to_consume(func.consumes.clone(), node.clone()) {
                    Consumed::AllTokens(new_edges) => {
                        step_executed_at_least_once = true;
                        self.edges.extend(new_edges);
                        if func.produces.is_empty() {
                            next_pools.push(pool.clone());
                        } else {
                            for branch in &func.produces {
                                let mut branched_pool = pool.clone();
                                branched_pool.produce(branch.clone(), node.clone());
                                next_pools.push(branched_pool);
                            }
                        }
                    }
                    Consumed::SomeTokens(missing) => {
                        next_pools.push(pool.clone());
                        for m in missing {
                            missing_tokens_examples.insert(m.kind.name().to_string());
                        }
                    }
                }
            }

            if !step_executed_at_least_once && !func.consumes.is_empty() {
                let missing_list: Vec<String> = missing_tokens_examples.into_iter().collect();
                let msg = format!(
                    "Flow Error: Function '{}' could not execute. Missing inputs: [{}]",
                    func.name,
                    missing_list.join(", ")
                );

                self.diagnostics.push(DiagnosticWithContext {
                    file_id: step.span.file_id,
                    span: Some(step.span),
                    message: msg,
                    severity: DiagnosticSeverity::ERROR, // STRICT MODE: Starvation is fatal
                    tags: vec![],
                });
            }

            self.pools = next_pools;
        }

        // 3. Process Leftovers and Lazily Add Boundary Nodes
        let mut has_terminal_flow = false;
        let mut has_error_flow = false;

        for pool in &self.pools {
            let leftovers = pool.get_leftover_tokens();

            // Check for valid leftovers (FinalNode)
            for token in leftovers.variables.into_iter().chain(leftovers.constants) {
                if let Some(origin) = pool.token_to_origin_node.get(&token.uid) {
                    has_terminal_flow = true;
                    self.edges.push(Edge {
                        from_node_uid: origin.uid,
                        to_node_uid: final_node.uid,
                        token,
                        relation: EdgeRelation::TerminalFlow,
                    });
                }
            }

            // Check for error leftovers (FatalErrors)
            for err in leftovers.errors {
                if let Some(origin) = pool.token_to_origin_node.get(&err.uid) {
                    has_error_flow = true;
                    self.edges.push(Edge {
                        from_node_uid: origin.uid,
                        to_node_uid: fatal_node.uid,
                        token: err.clone(),
                        relation: EdgeRelation::ErrorFlow,
                    });

                    // STRICT MODE: Unhandled errors are warnings.
                    // Locate the function definition that produced this error to attach the warning.
                    if let Some(meta) = structure.symbol_table.get(&origin.function.uid) {
                        self.diagnostics.push(DiagnosticWithContext {
                            file_id: meta.definition_span.file_id,
                            span: Some(meta.definition_span),
                            message: format!(
                                "Unhandled Error: '{}' is produced by '{}' but never consumed (rescued).",
                                err.kind.name(),
                                origin.function.name
                            ),
                            severity: DiagnosticSeverity::WARNING,
                            tags: vec![],
                        });
                    }
                }
            }
        }

        // 4. Finalize Nodes List
        // Add InitialNode only if it has outgoing edges
        if self
            .edges
            .iter()
            .any(|e| e.from_node_uid == initial_node.uid)
        {
            // Insert at the beginning for aesthetics
            self.nodes.insert(0, initial_node.clone());
        }

        // Add FinalNode if used
        if has_terminal_flow {
            self.nodes.push(final_node.clone());
        }

        // Add FatalErrors if used
        if has_error_flow {
            self.nodes.push(fatal_node.clone());
        }

        if self.deduplicate_edges {
            let mut seen = HashSet::new();
            self.edges
                .retain(|e| seen.insert((e.from_node_uid, e.to_node_uid, e.token.uid)));
        }

        Graph {
            nodes: self.nodes.iter().map(|n| (**n).clone()).collect(),
            edges: self.edges.clone(),
        }
    }
}
