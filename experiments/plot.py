import json
from enum import Enum
from typing import List, Optional
from pydantic import BaseModel

# ... (Original Function/Token classes remain exactly as before) ...


def generate_graph(json_input_file: str, html_output_file: str = "architecture.html"):
    with open(json_input_file, "r", encoding="utf-8") as f:
        data = json.load(f)

    vis_nodes, vis_edges, groups, name_to_uid = [], [], set(), {}

    # 1. Process Nodes
    for n in data.get("nodes", []):
        func_data = n["function"]
        uid, name = n["uid"], func_data["name"]
        name_to_uid[name] = uid
        group_name = func_data.get("group")["name"] if func_data.get("group") else None
        if group_name:
            groups.add(group_name)

        # Color Logic
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

    # 2. Process Edges
    for e in data.get("edges", []):
        u, v = (
            name_to_uid.get(e["origin_function"]["name"]),
            name_to_uid.get(e["destination_function"]["name"]),
        )
        if u is not None and v is not None:
            token = e["token"]
            kind = list(token["kind"].keys())[0]
            name = token["kind"][kind]["name"]
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
                    "label": f"[{name}]" if is_many else name,
                    "color": color,
                    "width": width * 5.0 if is_many else width,
                    "dashes": dash,
                    "font": {"size": 12, "color": "#e0e0e0", "strokeWidth": 0},
                    "arrows": "to",
                }
            )

    # 3. Generate HTML with Native Dark Theme
    html_template = f"""
    <!DOCTYPE html>
    <html style="color-scheme: dark;">
    <head>
        <script type="text/javascript" src="https://unpkg.com/vis-network/standalone/umd/vis-network.min.js"></script>
        <style type="text/css">
            body {{ background-color: #0b0e14; color: #e0e0e0; margin: 0; display: flex; font-family: sans-serif; }}
            #mynetwork {{ flex-grow: 1; height: 100vh; }}
            #config {{ width: 320px; height: 100vh; overflow-y: auto; background: #161b22; border-left: 1px solid #333; }}
            
            /* Simple Dark Theme overrides for Vis.js specific labels */
            .vis-configuration-wrapper {{ color: #e0e0e0 !important; padding: 10px; }}
            .vis-config-item {{ background: none !important; border: none !important; }}
            .vis-config-label {{ color: #bbb !important; }}
            .vis-config-header {{ color: #58a6ff !important; font-weight: bold; margin-top: 10px; border-bottom: 1px solid #333; }}
            
            /* Dark Navigation Buttons */
            .vis-network .vis-navigation .vis-button {{ background-color: #21262d; border: 1px solid #444; border-radius: 4px; }}
        </style>
    </head>
    <body>
    <div id="mynetwork"></div>
    <div id="config"></div>
    <script type="text/javascript">
        const nodes = new vis.DataSet({json.dumps(vis_nodes)});
        const edges = new vis.DataSet({json.dumps(vis_edges)});
        const data = {{ nodes, edges }};
        const options = {{
            physics: {{ solver: 'forceAtlas2Based', forceAtlas2Based: {{ gravitationalConstant: -100, springLength: 10, avoidOverlap: 1 }} }},
            interaction: {{ navigationButtons: true, keyboard: true }},
            configure: {{ enabled: true, container: document.getElementById('config'), showButton: false }}
        }};
        const network = new vis.Network(document.getElementById('mynetwork'), data, options);

        // Clustering Logic
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
    print(f"Dark graph generated: {html_output_file}")


if __name__ == "__main__":
    generate_graph("architecture.json", "architecture.html")
