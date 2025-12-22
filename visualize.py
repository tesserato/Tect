import json
import networkx as nx
import matplotlib.pyplot as plt

# 1. Load the graph data
with open('graph.json', 'r') as f:
    data = json.load(f)

# 2. Create Directed Graph
G = nx.DiGraph()

# 3. Define colors for different kinds
color_map = {
    "Data": "#3498db",      # Blue
    "Error": "#e74c3c",     # Red
    "Function": "#2ecc71",  # Green
    "Variable": "#95a5a6",  # Gray
    "Side Effect": "#f1c40f" # Yellow
}

# 4. Add Nodes
for node in data['nodes']:
    G.add_node(node['id'], label=node['label'], kind=node['kind'])

# 5. Add Edges
for edge in data['edges']:
    G.add_edge(edge['source'], edge['target'], label=edge['relation'])

# 6. Visualization Settings
plt.figure(figsize=(12, 8))
pos = nx.spring_layout(G, k=1.5) # k adjusts spacing

# Draw Nodes by kind
for kind, color in color_map.items():
    nodes = [n for n, attr in G.nodes(data=True) if attr.get('kind') == kind]
    nx.draw_networkx_nodes(G, pos, nodelist=nodes, node_color=color, node_size=2000, alpha=0.9)

# Draw Edges and Labels
nx.draw_networkx_edges(G, pos, width=1.5, alpha=0.5, edge_color="gray", arrowsize=20)
nx.draw_networkx_labels(G, pos, labels=nx.get_node_attributes(G, 'label'), font_size=10, font_weight="bold")

# Optional: Draw Edge Relation labels (e.g., "argument_flow")
edge_labels = nx.get_edge_attributes(G, 'label')
nx.draw_networkx_edge_labels(G, pos, edge_labels=edge_labels, font_size=7)

plt.title("Tect Architecture Graph")
plt.axis('off')
plt.tight_layout()

# 7. Show or Save
print("Saving graph to architecture.png...")
plt.savefig("architecture.png")
plt.show()