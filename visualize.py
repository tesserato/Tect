import json
import networkx as nx
import matplotlib.pyplot as plt
import matplotlib.patches as mpatches

# 1. Load Graph
with open("graph.json", "r") as f:
    data = json.load(f)

G = nx.MultiDiGraph()  # MultiDiGraph allows multiple edges between same nodes

color_map = {
    "Data": "#3498db",  # Blue
    "Error": "#e74c3c",  # Red
    "Function": "#2ecc71",  # Green
    "Variable": "#95a5a6",  # Gray
    "Side Effect": "#f1c40f",
}

# 2. Build Graph
for node in data["nodes"]:
    G.add_node(node["id"], label=node["label"], kind=node["kind"])

for edge in data["edges"]:
    G.add_edge(edge["source"], edge["target"], label=edge["relation"])

# 3. Layout and Styling
plt.figure(figsize=(16, 10), facecolor="white")
pos = nx.kamada_kawai_layout(G)

# 4. Draw Nodes
for kind, color in color_map.items():
    nodes = [n for n, attr in G.nodes(data=True) if attr.get("kind") == kind]
    nx.draw_networkx_nodes(
        G,
        pos,
        nodelist=nodes,
        node_color=color,
        node_size=3000,
        alpha=0.8,
        edgecolors="white",
        linewidths=2,
    )

# 5. Draw Labels (Nodes)
labels = {n: attr["label"] for n, attr in G.nodes(data=True)}
nx.draw_networkx_labels(
    G, pos, labels=labels, font_size=11, font_weight="bold", font_family="sans-serif"
)

# 6. Draw Edges with Curves
ax = plt.gca()
for edge in G.edges(data=True, keys=True):
    source, target, key, attr = edge
    rad = 0.1 * (key + 1)

    style = "solid"
    color = "#bdc3c7"
    if attr["label"] == "output_type":
        color = "#95a5a6"
    if attr["label"] == "argument_flow":
        style = "dashed"

    ax.annotate(
        "",
        xy=pos[target],
        xycoords="data",
        xytext=pos[source],
        textcoords="data",
        arrowprops=dict(
            arrowstyle="->",
            color=color,
            connectionstyle=f"arc3,rad={rad}",
            shrinkA=25,
            shrinkB=25,
            patchA=None,
            patchB=None,
            mutation_scale=20,
            lw=1.5,
            ls=style,
        ),
    )

# 7. Edge Labels (Relations)
edge_labels = {(u, v): d["label"] for u, v, d in G.edges(data=True)}
nx.draw_networkx_edge_labels(
    G, pos, edge_labels=edge_labels, font_size=8, label_pos=0.5, alpha=0.7
)

# 8. Create Legend
legend_handles = [
    mpatches.Patch(color=color, label=kind) for kind, color in color_map.items()
]
plt.legend(
    handles=legend_handles,
    loc="upper left",
    title="Node Types",
    frameon=True,
    fontsize=10,
)

plt.title("Tect Architecture Flow", fontsize=18, pad=20, fontweight="bold")
plt.axis("off")
plt.tight_layout()

print("Updating architecture.png...")
plt.savefig("architecture.png", dpi=300)
plt.show()
