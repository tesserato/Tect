import json
import networkx as nx
import matplotlib.pyplot as plt
import matplotlib.patches as mpatches

# 1. Load Graph
with open('graph.json', 'r') as f:
    data = json.load(f)

G = nx.MultiDiGraph()

# Professional Color Palette
color_map = {
    "Data": "#2ecc71",      # Green
    "Function": "#95a5a6",  # Gray
    "Error": "#e74c3c",     # Red
    "Variable": "#34495e",  # Deep Blue-Gray
    "Logic": "#f39c12",     # Orange
    "Side Effect": "#f39c12"
}

# 2. Build Nodes with Layering and Grouping
groups = sorted(list(set(node['group'] for node in data['nodes'])))
group_to_idx = {group: i for i, group in enumerate(groups)}

for node in data['nodes']:
    kind = node['kind']
    # Layers: 0: Types/Errors, 1: Functions, 2: Variables/Logic
    if kind in ["Data", "Error"]:
        layer = 0
    elif kind == "Function":
        layer = 1
    else:
        layer = 2
    
    G.add_node(node['id'], 
               label=node['label'], 
               kind=kind, 
               layer=layer, 
               group=node['group'])

# 3. Filter Edges (Class-to-Object)
allowed_relations = ["type_definition", "result_flow"]
for edge in data['edges']:
    if edge['relation'] in allowed_relations:
        G.add_edge(edge['source'], edge['target'], label=edge['relation'])

# 4. Clustered Hierarchical Layout
# We use the group index to shift the horizontal position
pos = nx.multipartite_layout(G, subset_key="layer", align='horizontal')

# Adjust layout for vertical flow and group clustering
group_shift = 5.0  # Horizontal distance between groups
for node_id, coords in pos.items():
    node_group = G.nodes[node_id]['group']
    shift = group_to_idx[node_group] * group_shift
    
    pos[node_id][0] = (coords[0] * 2.0) + shift  # Horizontal spread + grouping
    pos[node_id][1] = -coords[1] * 3.0           # Vertical flip for top-to-bottom

# 5. Initialize Plot
plt.figure(figsize=(20, 12), facecolor='white')
ax = plt.gca()

# 6. Draw Group Backgrounds (Optional but helpful)
# Here we just label the group areas
for group in groups:
    group_nodes = [n for n, attr in G.nodes(data=True) if attr['group'] == group]
    if not group_nodes: continue
    xs = [pos[n][0] for n in group_nodes]
    center_x = sum(xs) / len(xs)
    plt.text(center_x, 0.5, f"GROUP: {group.upper()}", 
             ha='center', fontsize=12, fontweight='bold', alpha=0.3, color="#7f8c8d")

# 7. Draw Edges
for u, v, key, attr in G.edges(data=True, keys=True):
    rad = 0.1 * (key + 1)
    ax.annotate("",
                xy=pos[v], xycoords='data',
                xytext=pos[u], textcoords='data',
                arrowprops=dict(arrowstyle="->", 
                                color="#bdc3c7",
                                connectionstyle=f"arc3,rad={rad}",
                                shrinkA=20, shrinkB=20,
                                mutation_scale=15, 
                                lw=1.0, 
                                alpha=0.4))

# 8. Draw Nodes
for kind, color in color_map.items():
    nodes = [n for n, attr in G.nodes(data=True) if attr.get('kind') == kind]
    if not nodes: continue
    nx.draw_networkx_nodes(G, pos, nodelist=nodes, node_color=color, 
                           node_size=2500, alpha=0.9, edgecolors='white', linewidths=1)

# 9. Labels
labels = {n: attr['label'] for n, attr in G.nodes(data=True)}
nx.draw_networkx_labels(G, pos, labels=labels, font_size=7, 
                        font_weight='bold', font_family='sans-serif', font_color="white")

# 10. Legend
type_handles = [mpatches.Patch(color=color, label=kind) for kind, color in color_map.items() 
                if any(attr['kind'] == kind for _, attr in G.nodes(data=True))]
plt.legend(handles=type_handles, loc='upper left', title="Object Hierarchy", frameon=True, fontsize=8)

plt.title("Tect Architecture: Clustered System Map", fontsize=18, pad=30, fontweight='bold')
plt.axis('off')
plt.tight_layout()

print("Updating architecture.png...")
plt.savefig("architecture.png", dpi=300, bbox_inches='tight')
plt.show()