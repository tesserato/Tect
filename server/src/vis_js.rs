//! # Vis.js Interactive Exporter
//!
//! Ports the logic from `plot.py` to Rust. Generates an interactive HTML/JS
//! visualization using vis-network. Supports clustering, live configuration,
//! and draggable sidebars.

use crate::models::{Cardinality, Edge, Graph, Kind, Node};
use serde::Serialize;
use std::collections::HashSet;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VisColor {
    background: String,
    border: String,
    highlight: VisHighlight,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VisHighlight {
    background: String,
    border: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VisFont {
    color: String,
    size: u32,
    stroke_width: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VisNode {
    id: u32,
    label: String,
    shape: String,
    margin: u32,
    cluster_group: Option<String>,
    color: VisColor,
    border_width: u32,
    font: VisFont,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VisEdge {
    from: u32,
    to: u32,
    label: String,
    color: String,
    width: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    dashes: Option<Vec<u32>>,
    font: VisFont,
    arrows: String,
}

/// Converts the logical Tect Graph into a format consumed by vis-network.js
pub fn generate_interactive_html(graph: &Graph) -> String {
    let mut vis_nodes = Vec::new();
    let mut vis_edges = Vec::new();
    let mut groups = HashSet::new();

    for n in &graph.nodes {
        let group_name = n.function.group.as_ref().map(|g| g.name.clone());
        if let Some(ref g) = group_name {
            groups.insert(g.clone());
        }

        let bg = if n.is_artificial_error_termination {
            "#dc2626" // Red
        } else if n.is_artificial_graph_start || n.is_artificial_graph_end {
            "#059669" // Emerald
        } else {
            "#1d4ed8" // Blue
        };

        let border = if group_name.is_some() {
            "#fbbf24"
        } else {
            "#ffffff"
        };

        vis_nodes.push(VisNode {
            id: n.uid,
            label: format!(" {} ", n.function.name),
            shape: "box".to_string(),
            margin: 10,
            cluster_group: group_name,
            color: VisColor {
                background: bg.to_string(),
                border: border.to_string(),
                highlight: VisHighlight {
                    background: bg.to_string(),
                    border: border.to_string(),
                },
            },
            border_width: if border == "#fbbf24" { 2 } else { 1 },
            font: VisFont {
                color: "#ffffff".to_string(),
                size: 14,
                stroke_width: 0,
            },
        });
    }

    for e in &graph.edges {
        let kind_str = match &e.token.kind {
            Kind::Variable(_) => "Variable",
            Kind::Constant(_) => "Constant",
            Kind::Error(_) => "Error",
        };

        let t_name = match &e.token.kind {
            Kind::Variable(v) => &v.name,
            Kind::Constant(c) => &c.name,
            Kind::Error(er) => &er.name,
        };

        let is_many = e.token.cardinality == Cardinality::Collection;

        let (mut width, dashes, color) = match kind_str {
            "Variable" => (1.0, None, "#818cf8"),
            "Constant" => (1.0, Some(vec![5, 8]), "#818cf8"),
            _ => (1.0, None, "#f87171"), // Error
        };

        if is_many {
            width *= 5.0;
        }

        vis_edges.push(VisEdge {
            from: e.from_node_uid,
            to: e.to_node_uid,
            label: if is_many {
                format!("[{}]", t_name)
            } else {
                t_name.clone()
            },
            color: color.to_string(),
            width,
            dashes,
            font: VisFont {
                color: "#e0e0e0".to_string(),
                size: 12,
                stroke_width: 0,
            },
            arrows: "to".to_string(),
        });
    }

    let nodes_json = serde_json::to_string(&vis_nodes).unwrap();
    let edges_json = serde_json::to_string(&vis_edges).unwrap();
    let groups_json = serde_json::to_string(&groups.into_iter().collect::<Vec<_>>()).unwrap();

    format!(
        r#"
    <!DOCTYPE html>
    <html style="color-scheme: dark;">
    <head>
        <script type="text/javascript" src="https://unpkg.com/vis-network/standalone/umd/vis-network.min.js"></script>
        <style type="text/css">
            body {{ background-color: #0b0e14; color: #e0e0e0; margin: 0; display: flex; font-family: sans-serif; height: 100vh; overflow: hidden; }}
            #mynetwork {{ flex-grow: 1; height: 100vh; }}
            #resizer {{ width: 6px; cursor: col-resize; background-color: #30363d; transition: background 0.2s; z-index: 10; }}
            #resizer:hover {{ background-color: #58a6ff; }}

            #config {{ width: 350px; min-width: 250px; height: 100vh; overflow-y: auto; background: #161b22; flex-shrink: 0; display: flex; flex-direction: column; }}
            #config-controls {{ flex-grow: 1; }}
            
            #options-export {{ padding: 15px; background: #0d1117; border-top: 2px solid #30363d; flex-shrink: 0; }}
            #options-export h3 {{ margin-top: 0; font-size: 14px; color: #58a6ff; }}
            #options-code {{ 
                background: #161b22; padding: 10px; border-radius: 4px; font-family: monospace; 
                font-size: 11px; max-height: 200px; overflow: auto; white-space: pre-wrap;
                border: 1px solid #30363d; color: #8b949e;
            }}
            #copy-btn {{
                margin-top: 10px; width: 100%; padding: 8px; background: #238636; color: white;
                border: none; border-radius: 4px; cursor: pointer; font-weight: bold;
            }}
            #copy-btn:hover {{ background: #2ea043; }}

            .vis-configuration-wrapper {{ color: #e0e0e0 !important; padding: 10px; }}
            .vis-config-item {{ background: none !important; border: none !important; }}
            .vis-config-label {{ color: #bbb !important; }}
            .vis-config-header {{ color: #58a6ff !important; font-weight: bold; margin-top: 10px; border-bottom: 1px solid #333; }}
            .vis-network .vis-navigation .vis-button {{ background-color: #21262d; border: 1px solid #444; border-radius: 4px; }}
        </style>
    </head>
    <body>
    <div id="mynetwork"></div>
    <div id="resizer"></div>
    <div id="config">
        <div id="config-controls"></div>
        <div id="options-export">
            <h3>Current Options (JSON)</h3>
            <div id="options-code">Modify a control to see JSON...</div>
            <button id="copy-btn">Copy Options</button>
        </div>
    </div>

    <script type="text/javascript">
        const configContainer = document.getElementById('config');
        const controlsDiv = document.getElementById('config-controls');
        const resizer = document.getElementById('resizer');
        const optionsCode = document.getElementById('options-code');
        const copyBtn = document.getElementById('copy-btn');
        
        const nodes = new vis.DataSet({nodes_json});
        const edges = new vis.DataSet({edges_json});
        
        let isResizing = false;
        resizer.addEventListener('mousedown', () => isResizing = true);
        document.addEventListener('mousemove', (e) => {{
            if (!isResizing) return;
            const newWidth = window.innerWidth - e.clientX;
            if (newWidth > 200 && newWidth < 900) configContainer.style.width = newWidth + 'px';
        }});
        document.addEventListener('mouseup', () => isResizing = false);

        function updateOptionsDisplay(params) {{
            optionsCode.innerText = JSON.stringify(params, null, 2);
        }}

        copyBtn.addEventListener('click', () => {{
            const text = optionsCode.innerText;
            if (text.startsWith('Modify')) return;
            navigator.clipboard.writeText(text).then(() => {{
                const originalText = copyBtn.innerText;
                copyBtn.innerText = "Copied!";
                setTimeout(() => {{ copyBtn.innerText = originalText; }}, 1500);
            }});
        }});

        const options = {{
            physics: {{ solver: 'forceAtlas2Based', forceAtlas2Based: {{ gravitationalConstant: -100, springLength: 10, avoidOverlap: 1, damping: 0.75 }} }},
            interaction: {{ navigationButtons: true, keyboard: true }},
            configure: {{ enabled: true, container: controlsDiv, showButton: false }}
        }};
        const network = new vis.Network(document.getElementById('mynetwork'), {{ nodes, edges }}, options);

        network.on("configChange", (params) => updateOptionsDisplay(params));

        const clusterBy = (g) => ({{
            joinCondition: (n) => n.clusterGroup === g,
            clusterNodeProperties: {{ id: 'c:'+g, label: g, shape: 'box', margin: 10, color: {{ background: '#fbbf24', border: '#fff' }}, font: {{ color: '#000' }} }}
        }});
        {groups_json}.forEach(g => network.cluster(clusterBy(g)));

        network.on("click", (p) => {{
            if (p.nodes.length > 0) {{
                let id = p.nodes[0];
                if (network.isCluster(id)) network.openCluster(id);
                else {{ let d = nodes.get(id); if (d && d.clusterGroup) network.cluster(clusterBy(d.clusterGroup)); }}
            }}
        }});
    </script>
    </body>
    </html>
    "#
    )
}
