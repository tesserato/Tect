//! # Tect Logic Engine
//!
//! Orchestrates the architectural simulation by consuming a [ProgramStructure].
//! It tracks the movement of tokens through pools and handles branching.

use crate::models::*;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub enum Consumed {
    AllTokens(Vec<Edge>),
    SomeTokens(Vec<Token>),
}

pub struct Leftovers {
    pub variables: Vec<Token>,
    pub errors: Vec<Token>,
    pub constants: Vec<Token>,
}

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
    pub fn new(initial_requirements: Vec<Token>, initial_node: Arc<Node>) -> Self {
        let mut variables = Vec::new();
        let mut errors = Vec::new();
        let mut constants = Vec::new();
        let mut token_to_origin_node = HashMap::new();

        for token in initial_requirements {
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

            let matched = pool
                .iter()
                .find(|t| t.kind == req.kind && !consumed_in_step.contains(t))
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
                        relation: "data_flow".to_string(),
                    });
                    consumed_in_step.push(t);
                }
            }
        }

        let missing: Vec<Token> = requirements
            .iter()
            .filter(|req| !consumed_in_step.iter().any(|c| c.kind == req.kind))
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

pub struct Flow {
    pub nodes: Vec<Arc<Node>>,
    pub edges: Vec<Edge>,
    pub pools: Vec<TokenPool>,
    pub deduplicate_edges: bool,
}

impl Flow {
    pub fn new(deduplicate_edges: bool) -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            pools: Vec::new(),
            deduplicate_edges,
        }
    }

    pub fn simulate(&mut self, structure: &ProgramStructure) -> Graph {
        let initial_node = Arc::new(Node::new_artificial(
            "InitialNode".to_string(),
            true,
            false,
            false,
        ));
        self.nodes.push(initial_node.clone());

        if let Some(first_name) = structure.flow.first() {
            if let Some(func) = structure.catalog.get(first_name) {
                self.pools
                    .push(TokenPool::new(func.consumes.clone(), initial_node.clone()));
            }
        }

        for func_name in &structure.flow {
            let Some(func) = structure.catalog.get(func_name) else {
                continue;
            };
            let node = Arc::new(Node::new(func.clone()));
            self.nodes.push(node.clone());

            let mut next_pools = Vec::new();
            for pool in &mut self.pools {
                match pool.try_to_consume(func.consumes.clone(), node.clone()) {
                    Consumed::AllTokens(new_edges) => {
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
                    Consumed::SomeTokens(_) => {
                        next_pools.push(pool.clone());
                    }
                }
            }
            self.pools = next_pools;
        }

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
        self.nodes.push(final_node.clone());
        self.nodes.push(fatal_node.clone());

        for pool in &self.pools {
            let leftovers = pool.get_leftover_tokens();
            for token in leftovers.variables.into_iter().chain(leftovers.constants) {
                if let Some(origin) = pool.token_to_origin_node.get(&token.uid) {
                    self.edges.push(Edge {
                        from_node_uid: origin.uid,
                        to_node_uid: final_node.uid,
                        token,
                        relation: "terminal_flow".into(),
                    });
                }
            }
            for err in leftovers.errors {
                if let Some(origin) = pool.token_to_origin_node.get(&err.uid) {
                    self.edges.push(Edge {
                        from_node_uid: origin.uid,
                        to_node_uid: fatal_node.uid,
                        token: err,
                        relation: "error_flow".into(),
                    });
                }
            }
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
