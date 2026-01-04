import json
from enum import Enum
from typing import List, Optional
from pydantic import BaseModel


def generate_graph(json_input_file: str, html_output_file: str = "architecture.html"):
    with open(json_input_file, "r", encoding="utf-8") as f:
        data = json.load(f)

    vis_nodes, vis_edges, groups, name_to_uid = [], [], set(), {}

    for n in data.get("nodes", []):
        func_data = n["function"]
        uid, name = n["uid"], func_data["name"]
        name_to_uid[name] = uid
        group_name = func_data.get("group")["name"] if func_data.get("group") else None
        if group_name:
            groups.add(group_name)

        if n.get("is_artificial_error_termination"):
            bg = "#dc2626"
        elif n.get("is_artificial_graph_start") or n.get("is_artificial_graph_end"):
            bg = "#059669"
        else:
            bg = "#1d4ed8"

        vis_nodes.append(
            {
                "id": uid,
                "label": f" {name} ",
                "shape": "box",
                "margin": 10,
                "clusterGroup": group_name,
                "color": {
                    "background": bg,
                    "border": "#fbbf24" if group_name else "#ffffff",
                    "highlight": {
                        "background": bg,
                        "border": "#fbbf24" if group_name else "#ffffff",
                    },
                },
                "borderWidth": 2 if group_name else 1,
                "font": {"color": "#ffffff"},
            }
        )

    for e in data.get("edges", []):
        u, v = (
            name_to_uid.get(e["origin_function"]["name"]),
            name_to_uid.get(e["destination_function"]["name"]),
        )
        if u is not None and v is not None:
            token = e["token"]
            kind = list(token["kind"].keys())[0]
            t_name = token["kind"][kind]["name"]
            is_many = token.get("cardinality") == "Collection"
            width, dash, color = (
                (1.0, False, "#818cf8")
                if kind == "Variable"
                else (1.0, [5, 8], "#818cf8")
                if kind == "Constant"
                else (1.0, False, "#f87171")
            )
            vis_edges.append(
                {
                    "from": u,
                    "to": v,
                    "label": f"[{t_name}]" if is_many else t_name,
                    "color": color,
                    "width": width * 5.0 if is_many else width,
                    "dashes": dash,
                    "font": {"size": 12, "color": "#e0e0e0", "strokeWidth": 0},
                    "arrows": "to",
                }
            )

    html_template = f"""
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
            
            /* Export Options Section */
            #options-export {{ 
                padding: 15px; 
                background: #0d1117; 
                border-top: 2px solid #30363d; 
                flex-shrink: 0;
            }}
            #options-export h3 {{ margin-top: 0; font-size: 14px; color: #58a6ff; }}
            #options-code {{ 
                background: #161b22; 
                padding: 10px; 
                border-radius: 4px; 
                font-family: monospace; 
                font-size: 11px; 
                max-height: 200px; 
                overflow: auto; 
                white-space: pre-wrap;
                border: 1px solid #30363d;
                color: #8b949e;
            }}
            #copy-btn {{
                margin-top: 10px;
                width: 100%;
                padding: 8px;
                background: #238636;
                color: white;
                border: none;
                border-radius: 4px;
                cursor: pointer;
                font-weight: bold;
            }}
            #copy-btn:hover {{ background: #2ea043; }}
            #copy-btn:active {{ background: #238636; }}

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
        
        const nodes = new vis.DataSet({json.dumps(vis_nodes)});
        const edges = new vis.DataSet({json.dumps(vis_edges)});
        
        // --- 1. DRAGGABLE SIDEBAR LOGIC ---
        let isResizing = false;
        resizer.addEventListener('mousedown', () => isResizing = true);
        document.addEventListener('mousemove', (e) => {{
            if (!isResizing) return;
            const newWidth = window.innerWidth - e.clientX;
            if (newWidth > 200 && newWidth < 900) configContainer.style.width = newWidth + 'px';
        }});
        document.addEventListener('mouseup', () => isResizing = false);

        // --- 2. SCROLL PERSISTENCE ---
        let lastScrollTop = 0;
        configContainer.addEventListener('scroll', () => {{ if (configContainer.scrollTop > 0) lastScrollTop = configContainer.scrollTop; }}, {{passive: true}});
        new MutationObserver(() => {{ if (configContainer.scrollTop !== lastScrollTop) configContainer.scrollTop = lastScrollTop; }})
            .observe(controlsDiv, {{ childList: true, subtree: true }});

        // --- 3. OPTIONS EXPORT & COPY ---
        function updateOptionsDisplay(params) {{
            // Format JSON for display
            optionsCode.innerText = JSON.stringify(params, null, 2);
        }}

        copyBtn.addEventListener('click', () => {{
            const text = optionsCode.innerText;
            if (text.startsWith('Modify')) return;
            navigator.clipboard.writeText(text).then(() => {{
                const originalText = copyBtn.innerText;
                copyBtn.innerText = "Copied!";
                copyBtn.style.background = "#58a6ff";
                setTimeout(() => {{ 
                    copyBtn.innerText = originalText; 
                    copyBtn.style.background = "#238636";
                }}, 1500);
            }});
        }});

        // --- 4. GRAPH INITIALIZATION ---
        const options = {{
            physics: {{ solver: 'forceAtlas2Based', forceAtlas2Based: {{ gravitationalConstant: -100, springLength: 10, avoidOverlap: 1 }} }},
            interaction: {{ navigationButtons: true, keyboard: true }},
            configure: {{ enabled: true, container: controlsDiv, showButton: false }}
        }};
        const network = new vis.Network(document.getElementById('mynetwork'), {{ nodes, edges }}, options);

        // Capture live updates from the configurator
        network.on("configChange", (params) => {{
            updateOptionsDisplay(params);
        }});

        const clusterBy = (g) => ({{
            joinCondition: (n) => n.clusterGroup === g,
            clusterNodeProperties: {{ id: 'c:'+g, label: g, shape: 'box', margin: 10, color: {{ background: '#fbbf24', border: '#fff' }}, font: {{ color: '#000' }} }}
        }});
        {json.dumps(list(groups))}.forEach(g => network.cluster(clusterBy(g)));

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
    """

    with open(html_output_file, "w", encoding="utf-8") as f:
        f.write(html_template)
    print(f"Interactive graph with export generated: {html_output_file}")


if __name__ == "__main__":
    generate_graph("architecture.json", "architecture.html")
