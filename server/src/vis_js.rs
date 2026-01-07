//! # Vis.js Data Translator & Exporter
//!
//! Responsible for translating the logical architecture graph into
//! a visual representation compatible with Vis.js.

use crate::models::{Cardinality, Graph, Kind};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Represents the visual payload sent to the Webview or injected into HTML.
#[derive(Serialize, Deserialize, Clone)]
pub struct VisData {
    pub nodes: Vec<VisNode>,
    pub edges: Vec<VisEdge>,
    pub groups: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VisColor {
    pub background: String,
    pub border: String,
    pub highlight: VisHighlight,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VisHighlight {
    pub background: String,
    pub border: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VisFont {
    pub color: String,
    pub size: u32,
    pub face: String,
    pub stroke_width: u32,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VisNode {
    pub id: u32,
    pub label: String,
    pub shape: String,
    pub margin: u32,
    pub cluster_group: Option<String>,
    pub color: VisColor,
    pub border_width: u32,
    pub font: VisFont,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VisEdge {
    pub from: u32,
    pub to: u32,
    pub label: String,
    pub color: String,
    pub width: f32,
    pub dashes: bool,
    pub arrows: String,
    pub font: VisFont,
}

/// Translates a logical Graph into visual VisData.
/// This is the "Single Source of Truth" for styling.
pub fn produce_vis_data(graph: &Graph) -> VisData {
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
            shape: "box".into(),
            margin: 10,
            cluster_group: group_name.clone(),
            color: VisColor {
                background: bg.into(),
                border: border.into(),
                highlight: VisHighlight {
                    background: bg.into(),
                    border: "#ffffff".into(),
                },
            },
            border_width: if group_name.is_some() { 2 } else { 1 },
            font: VisFont {
                color: "#ffffff".into(),
                size: 14,
                face: "sans-serif".into(),
                stroke_width: 0,
            },
        });
    }

    for e in &graph.edges {
        let is_many = e.token.cardinality == Cardinality::Collection;
        let t_name = match &e.token.kind {
            Kind::Constant(c) => &c.name,
            Kind::Variable(v) => &v.name,
            Kind::Error(er) => &er.name,
        };

        let color = if matches!(e.token.kind, Kind::Error(_)) {
            "#f87171"
        } else {
            "#818cf8"
        };

        vis_edges.push(VisEdge {
            from: e.from_node_uid,
            to: e.to_node_uid,
            label: if is_many {
                format!("[{}]", t_name)
            } else {
                t_name.clone()
            },
            color: color.into(),
            width: if is_many { 5.0 } else { 1.5 },
            dashes: matches!(e.token.kind, Kind::Constant(_)),
            arrows: "to".into(),
            font: VisFont {
                color: "#ffffff".into(),
                size: 11,
                face: "monospace".into(),
                stroke_width: 0,
            },
        });
    }

    VisData {
        nodes: vis_nodes,
        edges: vis_edges,
        groups: groups.into_iter().collect(),
    }
}

/// Generates a complete standalone HTML file.
/// Used by the CLI `build` command for portable exports.
pub fn generate_interactive_html(graph: &Graph) -> String {
    let data = produce_vis_data(graph);
    let nodes_json = serde_json::to_string(&data.nodes).unwrap();
    let edges_json = serde_json::to_string(&data.edges).unwrap();
    let groups_json = serde_json::to_string(&data.groups).unwrap();

    format!(
        r#"<!DOCTYPE html>
<html style="color-scheme: dark;">
<head>
    <meta charset="utf-8">
    <script type="text/javascript" src="https://unpkg.com/vis-network/standalone/umd/vis-network.min.js"></script>
    <style type="text/css">
        body {{ background-color: #0b0e14; color: #e0e0e0; margin: 0; display: flex; font-family: sans-serif; height: 100vh; overflow: hidden; }}
        #mynetwork {{ flex-grow: 1; height: 100vh; }}
        #resizer {{ width: 6px; cursor: col-resize; background-color: #30363d; transition: background 0.2s; z-index: 10; }}
        #resizer:hover {{ background-color: #58a6ff; }}
        #config {{ width: 350px; min-width: 250px; height: 100vh; overflow-y: auto; background: #161b22; flex-shrink: 0; display: flex; flex-direction: column; }}
        #config-controls {{ flex-grow: 1; }}
        .vis-configuration-wrapper {{ color: #e0e0e0 !important; padding: 10px; }}
        .vis-config-item {{ background: none !important; border: none !important; }}
        .vis-config-label {{ color: #bbb !important; }}
        .vis-config-header {{ color: #58a6ff !important; font-weight: bold; margin-top: 10px; border-bottom: 1px solid #333; }}
        .vis-network .vis-navigation .vis-button {{ background-color: #21262d; border: 1px solid #444; border-radius: 4px; }}
        #options-export {{ padding: 15px; background: #0d1117; border-top: 2px solid #30363d; flex-shrink: 0; }}
        #options-export h3 {{ margin-top: 0; font-size: 14px; color: #58a6ff; }}
        #options-code {{ background: #161b22; padding: 10px; border-radius: 4px; font-family: monospace; font-size: 11px; max-height: 200px; overflow: auto; white-space: pre-wrap; border: 1px solid #30363d; color: #8b949e; }}
        #copy-btn {{ margin-top: 10px; width: 100%; padding: 8px; background: #238636; color: white; border: none; border-radius: 4px; cursor: pointer; font-weight: bold; }}
        #copy-btn:hover {{ background: #2ea043; }}
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
    const nodes = new vis.DataSet({nodes_json});
    const edges = new vis.DataSet({edges_json});
    const groups = {groups_json};
    const container = document.getElementById('mynetwork');
    const configContainer = document.getElementById('config');
    const configControls = document.getElementById('config-controls');
    const optionsCode = document.getElementById('options-code');
    const copyBtn = document.getElementById('copy-btn');
    const resizer = document.getElementById('resizer');
    let isResizing = false;
    resizer.addEventListener('mousedown', () => isResizing = true);
    document.addEventListener('mousemove', (e) => {{
        if (!isResizing) return;
        const newWidth = window.innerWidth - e.clientX;
        if (newWidth > 200 && newWidth < 900) configContainer.style.width = newWidth + 'px';
    }});
    document.addEventListener('mouseup', () => isResizing = false);
    let lastScrollTop = 0;
    configContainer.addEventListener('scroll', () => {{ if (configContainer.scrollTop > 0) lastScrollTop = configContainer.scrollTop; }}, {{passive: true}});
    new MutationObserver(() => {{ if (configContainer.scrollTop !== lastScrollTop) configContainer.scrollTop = lastScrollTop; }})
        .observe(configControls, {{ childList: true, subtree: true }});
    const data = {{ nodes, edges }};
    const options = {{
        physics: {{ enabled: true, solver: 'forceAtlas2Based', forceAtlas2Based: {{ gravitationalConstant: -100, springLength: 10, avoidOverlap: 1, damping: 0.75 }} }},
        interaction: {{ navigationButtons: true, keyboard: true, hover: true }},
        configure: {{ enabled: true, container: configControls, showButton: false }}
    }};
    const network = new vis.Network(container, data, options);
    network.on("configChange", (params) => {{ optionsCode.innerText = JSON.stringify(params, null, 2); }});
    const clusterBy = (g) => ({{
        joinCondition: (n) => n.clusterGroup === g,
        clusterNodeProperties: {{ id: 'c:'+g, label: g, shape: 'box', margin: 10, color: {{ background: '#fbbf24', border: '#fff' }}, font: {{ color: '#000', size: 16, face: 'sans-serif', strokeWidth: 0 }} }}
    }});
    groups.forEach(g => network.cluster(clusterBy(g)));
    network.on("click", (p) => {{
        if (p.nodes.length > 0) {{
            let id = p.nodes[0];
            if (network.isCluster(id)) network.openCluster(id);
            else {{ let d = nodes.get(id); if (d && d.clusterGroup) network.cluster(clusterBy(d.clusterGroup)); }}
        }}
    }});
    copyBtn.addEventListener('click', () => {{
        navigator.clipboard.writeText(optionsCode.innerText).then(() => {{
            const originalText = copyBtn.innerText;
            copyBtn.innerText = "Copied!";
            setTimeout(() => {{ copyBtn.innerText = originalText; }}, 1500);
        }});
    }});
</script>
</body>
</html>"#
    )
}
