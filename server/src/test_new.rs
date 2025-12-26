use serde::Serialize;
use std::fs::File;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};

// --- 1. Global Counter ---
// Thread-safe counter for generating unique IDs
static FUNC_ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

fn next_uid() -> usize {
    FUNC_ID_COUNTER.fetch_add(1, Ordering::SeqCst)
}

// --- 2. Models ---

#[derive(Debug, Clone, PartialEq, Serialize)]
struct Type {
    name: String,
    is_mutable: bool,
    is_collection: bool,
}

impl Type {
    fn new(name: &str, is_mutable: bool, is_collection: bool) -> Self {
        Self {
            name: name.to_string(),
            is_mutable,
            is_collection,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct TypeInstance {
    #[serde(flatten)]
    inner: Type,
    origin_uid: Option<usize>,
    destination_uid: Option<usize>,
}

impl TypeInstance {
    fn from_type(t: &Type, origin: Option<usize>) -> Self {
        Self {
            inner: t.clone(),
            origin_uid: origin,
            destination_uid: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct Function {
    uid: usize,
    name: String,
    consumes: Vec<Type>,
    produces: Vec<Type>,
    is_special: bool,
}

impl Function {
    fn new(name: &str, consumes: Vec<Type>, produces: Vec<Type>) -> Self {
        Self {
            uid: next_uid(),
            name: name.to_string(),
            consumes,
            produces,
            is_special: false,
        }
    }

    fn special(name: &str, uid: usize) -> Self {
        Self {
            uid,
            name: name.to_string(),
            consumes: vec![],
            produces: vec![],
            is_special: true,
        }
    }
}

// --- 3. Logic Engine ---

fn process_flow(flow: Vec<Function>) -> (Vec<Function>, Vec<TypeInstance>) {
    let start_node = Function::special("Start", 0);
    let end_node = Function::special("End", 9999);

    let mut nodes = vec![start_node.clone()];
    let mut edges: Vec<TypeInstance> = Vec::new();

    // Initialize pool from the first function's requirements
    let mut pool: Vec<TypeInstance> = flow[0]
        .consumes
        .iter()
        .map(|t| TypeInstance::from_type(t, Some(start_node.uid)))
        .collect();

    for func in flow {
        nodes.push(func.clone());

        // 1. Consumption: Grab ALL matching instances from the pool
        let mut i = 0;
        while i < pool.len() {
            // Check if the current pool item's type is one of the types the function consumes
            let is_accepted = func.consumes.iter().any(|req| *req == pool[i].inner);

            if is_accepted {
                // Create the edge snapshot
                let mut edge = pool[i].clone();
                edge.destination_uid = Some(func.uid);
                edges.push(edge);

                // If mutable, remove it from the pool.
                // If immutable (like Settings), leave it for others to consume.
                if pool[i].inner.is_mutable {
                    pool.remove(i);
                    // Don't increment i; the next item shifted into the current index
                    continue;
                }
            }
            i += 1;
        }

        for t_prod in &func.produces {
            if !t_prod.is_mutable {
                pool.retain(|instance| instance.inner.name != t_prod.name);
            }
            pool.push(TypeInstance::from_type(t_prod, Some(func.uid)));
        }
    }

    for mut terminal_item in pool {
        terminal_item.destination_uid = Some(end_node.uid);
        edges.push(terminal_item);
    }
    nodes.push(end_node);

    (nodes, edges)
}

// --- 4. Visualization (Fixed String Handling) ---

fn generate_graph(nodes: Vec<Function>, edges: Vec<TypeInstance>) {
    let nodes_json = serde_json::to_string(&nodes).unwrap();

    let edge_data: Vec<_> = edges
        .into_iter()
        .map(|e| {
            let label = format!(
                "{}{}",
                e.inner.name,
                if e.inner.is_collection { "[]" } else { "" }
            );
            serde_json::json!({
                "from": e.origin_uid,
                "to": e.destination_uid,
                "label": label,
                "color": if e.inner.is_mutable { "#00ffcc" } else { "#444444" },
                "width": if e.inner.is_collection { 3 } else { 1 },
                "arrows": "to"
            })
        })
        .collect();

    let edges_json = serde_json::to_string(&edge_data).unwrap();

    // Use r##" string literal so that internal "#" doesn't close the string early
    // Use {{ }} for literal CSS/JS braces
    let html = format!(
        r##"
    <!DOCTYPE html>
    <html>
    <head>
        <script type="text/javascript" src="https://unpkg.com/vis-network/standalone/umd/vis-network.min.js"></script>
        <style>#graph {{ width: 100%; height: 900px; background: #121212; }}</style>
    </head>
    <body>
        <div id="graph"></div>
        <script>
            const nodes = new vis.DataSet({}.map(n => ({{
                id: n.uid, 
                label: n.name, 
                shape: "box", 
                color: n.is_special ? "#2921FF" : "#FF5722", 
                font: {{ color: "white" }}
            }})));
            const edges = new vis.DataSet({});
            new vis.Network(document.getElementById("graph"), {{ nodes, edges }}, {{}});
        </script>
    </body>
    </html>"##,
        nodes_json, edges_json
    );

    let mut file = File::create("architecture.html").expect("Unable to create file");
    file.write_all(html.as_bytes())
        .expect("Unable to write data");
    println!("Graph generated: architecture.html");
}

#[test]
fn main() {
    let initial_command = Type::new("InitialCommand", true, false);
    let path_to_config = Type::new("PathToConfiguration", true, false);
    let settings = Type::new("Settings", false, false);
    let templates = Type::new("SiteTemplates", false, false);
    let source_file = Type::new("SourceFile", false, true);
    let article = Type::new("Article", true, false);
    let html_type = Type::new("HTML", true, false);
    let fs_error = Type::new("FileSystemError", true, false);

    let flow = vec![
        Function::new(
            "ProcessInitialCommand",
            vec![initial_command],
            vec![settings.clone(), path_to_config.clone()],
        ),
        Function::new(
            "LoadConfiguration",
            vec![path_to_config],
            vec![settings.clone()],
        ),
        Function::new(
            "LoadTemplates",
            vec![settings.clone()],
            vec![templates.clone()],
        ),
        Function::new(
            "FindSourceFiles",
            vec![settings.clone()],
            vec![source_file.clone(), fs_error.clone()],
        ),
        Function::new(
            "ParseSource",
            vec![source_file],
            vec![article.clone(), fs_error.clone()],
        ),
        Function::new(
            "RenderArticle",
            vec![article, templates, settings],
            vec![html_type.clone()],
        ),
        Function::new("WriteHTML", vec![html_type], vec![fs_error]),
    ];

    let (nodes, edges) = process_flow(flow);
    generate_graph(nodes, edges);
}
