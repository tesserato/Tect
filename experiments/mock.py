import uuid
from typing import List, Dict, Any
from pydantic import BaseModel
from pyvis.network import Network


# --- 1. Models ---
class Type(BaseModel):
    name: str
    is_mutable: bool = True

    def __hash__(self):
        return hash((self.name, self.is_mutable))

    def __eq__(self, other):
        return (
            isinstance(other, Type)
            and self.name == other.name
            and self.is_mutable == other.is_mutable
        )


class Function(BaseModel):
    name: str
    consumes: List[Type]
    produces: List[Type]


class TokenInstance:
    def __init__(self, data_type: Type, is_input: bool = False):
        self.id = f"node_{uuid.uuid4().hex[:8]}"
        self.name = data_type.name
        self.is_mutable = data_type.is_mutable
        self.is_input = is_input


# --- 2. Original Definitions & Flow ---
InitialCommand = Type(name="InitialCommand")
PathToConfiguration = Type(name="PathToConfiguration")
SourceFile = Type(name="SourceFile")
Article = Type(name="Article")
HTML = Type(name="HTML")
Settings = Type(name="Settings", is_mutable=False)
SiteTemplates = Type(name="SiteTemplates", is_mutable=False)
FSError = Type(name="FileSystemError")

ProcessInitialCommand = Function(
    name="ProcessInitialCommand",
    consumes=[InitialCommand],
    produces=[Settings, PathToConfiguration],
)
LoadConfiguration = Function(
    name="LoadConfiguration", consumes=[PathToConfiguration], produces=[Settings]
)
LoadTemplates = Function(
    name="LoadTemplates", consumes=[Settings], produces=[SiteTemplates]
)
FindSourceFiles = Function(
    name="FindSourceFiles", consumes=[Settings], produces=[SourceFile, FSError]
)
ParseSource = Function(
    name="ParseSource", consumes=[SourceFile], produces=[Article, FSError]
)
RenderArticle = Function(
    name="RenderArticle", consumes=[Article, SiteTemplates, Settings], produces=[HTML]
)
WriteHTML = Function(name="WriteHTML", consumes=[HTML], produces=[FSError])

my_flow = [
    ProcessInitialCommand,
    LoadConfiguration,
    LoadTemplates,
    FindSourceFiles,
    ParseSource,
    RenderArticle,
    WriteHTML,
]


# --- 3. Logic Engine (Generates IR) ---
def process_flow(flow: List[Function]) -> List[Dict[str, Any]]:
    """
    Validates the flow and creates an Intermediate Representation.
    The starting pool is derived automatically from the first function's requirements.
    """
    if not flow:
        raise ValueError("Flow cannot be empty.")

    pool = [TokenInstance(t, is_input=True) for t in flow[0].consumes]
    ir = []

    print(f"{'='*20} PROCESSING ARCHITECTURE {'='*20}")

    for i, func in enumerate(flow, 1):
        consumed = []
        for req in func.consumes:
            token = next((t for t in pool if t.name == req.name), None)
            if not token:
                raise ValueError(
                    f"❌ Error at Step {i}: '{func.name}' requires '{req.name}'"
                )
            consumed.append(token)
            if token.is_mutable:
                pool.remove(token)

        produced = []
        for prod in func.produces:
            existing = next((t for t in pool if t.name == prod.name), None)
            if not prod.is_mutable and existing:
                produced.append(existing)
            else:
                new_token = TokenInstance(prod)
                pool.append(new_token)
                produced.append(new_token)

        ir.append(
            {
                "step": i,
                "function": func.name,
                "consumed": consumed,
                "produced": produced,
            }
        )
        print(f"Validated: {func.name}")

    return ir


# --- 4. Visualizer (Consumes IR) ---
def generate_graph(ir: List[Dict[str, Any]], filename="architecture.html"):
    net = Network(
        height="900px",
        width="100%",
        bgcolor="#121212",
        font_color="white",
        directed=True,
        layout=True,
    )
    added_tokens = set()

    for entry in ir:
        f_name = entry["function"]
        f_id = f"f_{entry['step']}"

        net.add_node(f_id, label=f_name, shape="diamond", color="#6fb1fc", size=25)

        for t in entry["consumed"] + entry["produced"]:
            if t.id not in added_tokens:
                color = (
                    "#ff7575"
                    if "Error" in t.name
                    else ("#fccb6f" if not t.is_mutable else "#8de3a1")
                )
                node_params = {
                    "label": t.name,
                    "shape": "dot",
                    "color": color,
                    "size": 15,
                }
                # if t.is_input:
                    # node_params.update({"x": -600, "fixed": True})
                    # node_params.update({"mass": 10})
                net.add_node(t.id, **node_params)
                added_tokens.add(t.id)

        # Implementation: Only edges exiting from immutable data are gray
        for t in entry["consumed"]:
            # If data is immutable (is_mutable=False), use gray. Else, use green flow color.
            edge_color = "#444444" if not t.is_mutable else "#8de3a1"
            net.add_edge(t.id, f_id, color=edge_color)

        # Edges exiting functions are always colored (aqua/green)
        for t in entry["produced"]:
            net.add_edge(f_id, t.id, color="#00ffcc")

    net.set_options("""
    var options = {
      "physics": {
        "barnesHut": { "gravitationalConstant": -9999, "springLength": 1,"avoidOverlap": 1 },
        "minVelocity": 0.75
      }
    }
    """)
    net.show(filename, notebook=False)
    print(f"\nVisual graph generated: {filename}")


# --- 5. Execution ---
ir_data = process_flow(my_flow)
generate_graph(ir_data)
