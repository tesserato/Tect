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
    pub token_to_initial_node: HashMap<Token, Arc<Node>>,
    pub functions_called_multiple_times: HashSet<u32>,
    pub constants_used_at_least_once: HashSet<u32>,
}

impl TokenPool {
    pub fn new(tokens: Vec<Token>, initial_node: Arc<Node>) -> Self {
        let mut variables = Vec::new();
        let mut errors = Vec::new();
        let mut constants = Vec::new();
        let mut token_to_initial_node = HashMap::new();

        for token in tokens {
            token_to_initial_node.insert(token.clone(), initial_node.clone());
            match &*token.kind {
                Kind::Variable(..) => variables.push(token),
                Kind::Error(..) => errors.push(token),
                Kind::Constant(..) => constants.push(token),
                _ => {}
            };
        }
        Self {
            variables,
            errors,
            constants,
            token_to_initial_node,
            functions_called_multiple_times: HashSet::new(),
            constants_used_at_least_once: HashSet::new(),
        }
    }

    pub fn produce(&mut self, tokens: Vec<Token>, initial_node: Arc<Node>) {
        for mut token in tokens {
            if self
                .functions_called_multiple_times
                .contains(&initial_node.uid)
            {
                token.cardinality = Cardinality::Collection;
            }
            self.token_to_initial_node
                .insert(token.clone(), initial_node.clone());
            match &*token.kind {
                Kind::Variable(..) => self.variables.push(token),
                Kind::Error(..) => self.errors.push(token),
                Kind::Constant(..) => self.constants.push(token),
                _ => {}
            };
        }
    }

    pub fn try_to_consume(&mut self, tokens: Vec<Token>, destination_node: Arc<Node>) -> Consumed {
        let mut edges = Vec::new();
        let mut consumed_tokens: Vec<Token> = Vec::new();
        let mut is_called_multiple_times = false;

        for requested_token in &tokens {
            let pool_to_check = match &*requested_token.kind {
                Kind::Variable(..) => &self.variables,
                Kind::Error(..) => &self.errors,
                Kind::Constant(..) => &self.constants,
                _ => continue,
            };

            for available_token in pool_to_check {
                if available_token.compare(requested_token)
                    && !consumed_tokens.contains(available_token)
                {
                    if let Some(node) = self.token_to_initial_node.get(available_token) {
                        if requested_token.cardinality == Cardinality::Unitary
                            && available_token.cardinality == Cardinality::Collection
                        {
                            is_called_multiple_times = true;
                        }
                        edges.push(Edge {
                            origin_function: node.function.clone(),
                            destination_function: destination_node.function.clone(),
                            token: available_token.clone(),
                            source: node.function.name.clone(),
                            target: destination_node.function.name.clone(),
                            relation: "data_flow".to_string(),
                        });
                        consumed_tokens.push(available_token.clone());
                        break;
                    }
                }
            }
        }

        let unconsumed_tokens: Vec<Token> = tokens
            .iter()
            .filter(|req_token| !consumed_tokens.iter().any(|c| c.compare(req_token)))
            .cloned()
            .collect();

        if unconsumed_tokens.is_empty() {
            if is_called_multiple_times {
                self.functions_called_multiple_times
                    .insert(destination_node.uid);
            }
            self.variables.retain(|t| !consumed_tokens.contains(t));
            self.errors.retain(|t| !consumed_tokens.contains(t));

            for token in &consumed_tokens {
                if let Kind::Constant(_) = &*token.kind {
                    self.constants_used_at_least_once.insert(token.uid);
                }
            }
            Consumed::AllTokens(edges)
        } else {
            Consumed::SomeTokens(unconsumed_tokens)
        }
    }

    pub fn get_leftover_tokens(&self) -> Leftovers {
        Leftovers {
            variables: self.variables.clone(),
            errors: self.errors.clone(),
            constants: self
                .constants
                .iter()
                .filter(|c| !self.constants_used_at_least_once.contains(&c.uid))
                .cloned()
                .collect(),
        }
    }
}

pub struct Flow {
    uid_counter: u32,
    pub nodes: Vec<Arc<Node>>,
    pub edges: Vec<Edge>,
    pub pools: Vec<TokenPool>,
}

impl Flow {
    pub fn new() -> Self {
        Self {
            uid_counter: 0,
            nodes: Vec::new(),
            edges: Vec::new(),
            pools: Vec::new(),
        }
    }

    pub fn process_flow(&mut self, functions: &[Arc<Function>]) -> (Vec<Arc<Node>>, Vec<Edge>) {
        let initial_node = Arc::new(Node {
            uid: 0,
            function: Arc::new(Function {
                name: "InitialNode".to_string(),
                documentation: Some("Artificial initial node".to_string()),
                consumes: vec![],
                produces: vec![],
                group: None,
            }),
            is_artificial_graph_start: true,
            is_artificial_graph_end: false,
            is_artificial_error_termination: false,
        });
        self.nodes.push(initial_node.clone());

        if let Some(first_func) = functions.first() {
            self.pools.push(TokenPool::new(
                first_func.consumes.clone(),
                initial_node.clone(),
            ));
        }

        for function in functions {
            self.uid_counter += 1;
            let node = Arc::new(Node {
                uid: self.uid_counter,
                function: function.clone(),
                is_artificial_graph_start: false,
                is_artificial_graph_end: false,
                is_artificial_error_termination: false,
            });
            self.nodes.push(node.clone());

            let mut new_pools: Vec<TokenPool> = Vec::new();
            for pool in &mut self.pools {
                match pool.try_to_consume(function.consumes.clone(), node.clone()) {
                    Consumed::AllTokens(new_edges) => {
                        self.edges.extend(new_edges);
                        for produced_tokens in &function.produces {
                            let mut new_pool = pool.clone();
                            new_pool.produce(produced_tokens.clone(), node.clone());
                            new_pools.push(new_pool);
                        }
                    }
                    Consumed::SomeTokens(_) => {
                        new_pools.push(pool.clone());
                    }
                }
            }
            self.pools = new_pools;
        }

        let final_node = Arc::new(Node {
            uid: self.uid_counter + 1,
            function: Arc::new(Function {
                name: "FinalNode".to_string(),
                documentation: Some("Artificial final node".to_string()),
                consumes: vec![],
                produces: vec![],
                group: None, 
            }),
            is_artificial_graph_start: false,
            is_artificial_graph_end: true,
            is_artificial_error_termination: false,
        });
        self.nodes.push(final_node.clone());

        let fatal_error_node = Arc::new(Node {
            uid: self.uid_counter + 2,
            function: Arc::new(Function {
                name: "FatalErrors".to_string(),
                documentation: Some("Artificial error termination node".to_string()),
                consumes: vec![],
                produces: vec![],
                group: None, // TODO: Consider grouping by error type
            }),
            is_artificial_graph_start: false,
            is_artificial_graph_end: false,
            is_artificial_error_termination: true,
        });
        self.nodes.push(fatal_error_node.clone());

        for pool in &mut self.pools {
            let leftovers = pool.get_leftover_tokens();
            for token in leftovers
                .variables
                .into_iter()
                .chain(leftovers.constants.into_iter())
            {
                if let Some(origin) = pool.token_to_initial_node.get(&token) {
                    self.edges.push(Edge {
                        origin_function: origin.function.clone(),
                        destination_function: final_node.function.clone(),
                        token,
                        source: origin.function.name.clone(),
                        target: final_node.function.name.clone(),
                        relation: "terminal_flow".into(),
                    });
                }
            }
            for err in leftovers.errors {
                if let Some(origin) = pool.token_to_initial_node.get(&err) {
                    self.edges.push(Edge {
                        origin_function: origin.function.clone(),
                        destination_function: fatal_error_node.function.clone(),
                        token: err,
                        source: origin.function.name.clone(),
                        target: fatal_error_node.function.name.clone(),
                        relation: "error_flow".into(),
                    });
                }
            }
        }

        (self.nodes.clone(), self.edges.clone())
    }
}
