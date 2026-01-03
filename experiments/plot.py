import json
from enum import Enum
from typing import List, Optional
from pydantic import BaseModel


# --- ORIGINAL CLASSES (Kept for compatibility) ---
class Cardinality(Enum):
    ONE = "1"
    MANY = "*"


class Type(BaseModel):
    name: str
    is_mutable: bool = True


class Token(BaseModel):
    name: str
    is_mutable: bool = True
    is_collection: bool = False
    origin_function_uid: Optional[int] = None
    destination_function_uid: Optional[int] = None


class Function(BaseModel):
    name: str
    uid: int
    consumes: List[Token]
    produces: List[Token]
    is_artificial_graph_start: bool = False
    is_artificial_graph_end: bool = False
    is_artificial_error_termination: bool = False


def generate_graph(json_input_file: str, html_output_file: str = "architecture.html"):
    with open(json_input_file, "r", encoding="utf-8") as f:
        data = json.load(f)

    vis_nodes = []
    vis_edges = []
    groups = set()
    name_to_uid = {}

    # 1. Process Nodes
    for n in data.get("nodes", []):
        func_data = n["function"]
        uid = n["uid"]
        name = func_data["name"]
        name_to_uid[name] = uid

        group_info = func_data.get("group")
        group_name = (
            group_info["name"]
            if (group_info and isinstance(group_info, dict))
            else None
        )

        if group_name:
            groups.add(group_name)

        # Color Logic
        if n.get("is_artificial_error_termination"):
            bg_color = "#dc2626"  # Red
        elif n.get("is_artificial_graph_start") or n.get("is_artificial_graph_end"):
            bg_color = "#059669"  # Emerald
        else:
            bg_color = "#1d4ed8"  # Blue

        # Requirement: Yellow border if in a group, otherwise white
        border_color = "#fbbf24" if group_name else "#ffffff"

        vis_nodes.append(
            {
                "id": uid,
                "label": f" {name} ",
                "title": func_data.get("documentation", ""),
                "shape": "box",
                "margin": 10,
                # RENAME 'group' to 'clusterGroup' to prevent Vis.js auto-coloring
                "clusterGroup": group_name,
                "color": {
                    "background": bg_color,
                    "border": border_color,
                    "highlight": {"background": bg_color, "border": border_color},
                    "hover": {"background": bg_color, "border": border_color},
                },
                "borderWidth": 2 if group_name else 1,
                "font": {"color": "#e0e0e0"},
            }
        )

    # 2. Process Edges
    for e in data.get("edges", []):
        u = name_to_uid.get(e["origin_function"]["name"])
        v = name_to_uid.get(e["destination_function"]["name"])

        if u is not None and v is not None:
            token = e["token"]
            kind_key = list(token["kind"].keys())[0]
            token_name = token["kind"][kind_key]["name"]
            is_many = token.get("cardinality") == "Collection"

            if kind_key == "Variable":
                width, dashes, color = 1.0, False, "#818cf8"
            elif kind_key == "Constant":
                width, dashes, color = 1.0, [5, 8], "#818cf8"
            else:  # Error
                width, dashes, color = 1.0, False, "#f87171"

            if is_many:
                width *= 5.0

            vis_edges.append(
                {
                    "from": u,
                    "to": v,
                    "label": f"[{token_name}]" if is_many else token_name,
                    "color": color,
                    "width": width,
                    "dashes": dashes,
                    "font": {"size": 12, "color": "#e0e0e0", "strokeWidth": 0},
                    "arrows": "to",
                }
            )

    # 3. Generate HTML Template
    html_template = f"""
    <!DOCTYPE html>
    <html>
    <head>
        <script type="text/javascript" src="https://unpkg.com/vis-network/standalone/umd/vis-network.min.js"></script>
        <style type="text/css">
            body {{ background-color: #0b0e14; margin: 0; padding: 0; overflow: hidden; }}
            #mynetwork {{ width: 100vw; height: 100vh; }}
        </style>
    </head>
    <body>
    <div id="mynetwork"></div>
    <script type="text/javascript">
        const nodes = new vis.DataSet({json.dumps(vis_nodes)});
        const edges = new vis.DataSet({json.dumps(vis_edges)});
        const groupNames = {json.dumps(list(groups))};

        const container = document.getElementById('mynetwork');
        const data = {{ nodes: nodes, edges: edges }};
        const options = {{
            physics: {{
                forceAtlas2Based: {{ theta: 0.1, gravitationalConstant: -105, springLength: 5, damping: 1, avoidOverlap: 1 }},
                solver: 'forceAtlas2Based',
                timestep: 0.01
            }}
        }};
        const network = new vis.Network(container, data, options);

        const clusterOptionsByGroup = (group) => ({{
            // Use our custom property name 'clusterGroup'
            joinCondition: (node) => node.clusterGroup === group,
            clusterNodeProperties: {{
                id: 'cluster:' + group,
                label: ' GROUP: ' + group + ' ',
                shape: 'box',
                margin: 10,
                color: {{ background: '#fbbf24', border: '#ffffff' }},
                font: {{ color: '#000000' }},
                allowSingleNodeCluster: false
            }}
        }});

        // Initial Clustering
        groupNames.forEach(group => network.cluster(clusterOptionsByGroup(group)));

        // Handle Click for Collapse/Expand
        network.on("click", function (params) {{
            if (params.nodes.length > 0) {{
                let nodeId = params.nodes[0];
                if (network.isCluster(nodeId)) {{
                    network.openCluster(nodeId);
                }} else {{
                    let nodeData = nodes.get(nodeId);
                    if (nodeData && nodeData.clusterGroup) {{
                        network.cluster(clusterOptionsByGroup(nodeData.clusterGroup));
                    }}
                }}
            }}
        }});
    </script>
    </body>
    </html>
    """

    with open(html_output_file, "w", encoding="utf-8") as f:
        f.write(html_template)
    print(f"Graph generated: {html_output_file}")


if __name__ == "__main__":
    generate_graph("architecture.json", "architecture.html")
