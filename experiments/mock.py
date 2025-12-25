import uuid
from typing import List, Dict, Any
from pydantic import BaseModel
from pyvis.network import Network


# --- 1. Models ---
class Type(BaseModel):
    name: str
    is_mutable: bool = True


class Output(BaseModel):
    """Wraps a Type with metadata about whether it's a collection."""

    data_type: Type
    is_collection: bool = False


class Function(BaseModel):
    name: str
    consumes: List[Type]
    produces: List[Output]  # Now uses Output wrapper


class TokenInstance:
    def __init__(
        self, data_type: Type, is_collection: bool = False, is_input: bool = False
    ):
        self.id = f"node_{uuid.uuid4().hex[:8]}"
        self.name = data_type.name
        self.is_mutable = data_type.is_mutable
        self.is_collection = is_collection
        self.is_input = is_input


# --- 2. Definitions with Multiplicity ---
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
    produces=[Output(data_type=Settings), Output(data_type=PathToConfiguration)],
)

LoadConfiguration = Function(
    name="LoadConfiguration",
    consumes=[PathToConfiguration],
    produces=[Output(data_type=Settings)],
)

LoadTemplates = Function(
    name="LoadTemplates",
    consumes=[Settings],
    produces=[Output(data_type=SiteTemplates)],
)

FindSourceFiles = Function(
    name="FindSourceFiles",
    consumes=[Settings],
    produces=[
        Output(data_type=SourceFile, is_collection=True),
        Output(data_type=FSError),
    ],
)

ParseSource = Function(
    name="ParseSource",
    consumes=[SourceFile],
    produces=[Output(data_type=Article), Output(data_type=FSError)],
)

RenderArticle = Function(
    name="RenderArticle",
    consumes=[Article, SiteTemplates, Settings],
    produces=[Output(data_type=HTML)],
)

WriteHTML = Function(
    name="WriteHTML", consumes=[HTML], produces=[Output(data_type=FSError)]
)

my_flow = [
    ProcessInitialCommand,
    LoadConfiguration,
    LoadTemplates,
    FindSourceFiles,
    ParseSource,
    RenderArticle,
    WriteHTML,
]


# --- 3. Logic Engine with Propagation ---
def process_flow(flow: List[Function]) -> List[Dict[str, Any]]:
    pool = [TokenInstance(t, is_input=True) for t in flow[0].consumes]
    ir = []

    for i, func in enumerate(flow, 1):
        consumed = []
        is_iterative_step = False  # Tracks if this function is running in a loop

        for req in func.consumes:
            token = next((t for t in pool if t.name == req.name), None)
            if not token:
                raise ValueError(f"Missing {req.name}")

            # If any input is a collection, the function runs multiple times
            if token.is_collection:
                is_iterative_step = True

            consumed.append(token)
            if token.is_mutable:
                pool.remove(token)

        produced = []
        for out in func.produces:
            # Engine Fix for State
            existing = next((t for t in pool if t.name == out.data_type.name), None)
            if not out.data_type.is_mutable and existing:
                produced.append(existing)
            else:
                # PROPAGATION: Output is a collection if the function explicitly says so
                # OR if the function is currently iterating over a consumed collection.
                is_coll = out.is_collection or is_iterative_step
                new_token = TokenInstance(out.data_type, is_collection=is_coll)
                pool.append(new_token)
                produced.append(new_token)

        ir.append(
            {
                "step": i,
                "function": func.name,
                "consumed": consumed,
                "produced": produced,
                "is_iterative": is_iterative_step,
            }
        )
    return ir


# --- 4. Visualizer (Multiplicity Styles) ---
def generate_graph(ir: List[Dict[str, Any]], filename="architecture_multi.html"):
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
        f_id = f"f_{entry['step']}"

        # Iterative functions get a thicker border (shadow)
        net.add_node(
            f_id,
            label=entry["function"],
            shape="diamond",
            color="#6fb1fc",
            size=25,
            borderWidth=4 if entry["is_iterative"] else 1,
        )

        for t in entry["consumed"] + entry["produced"]:
            if t.id not in added_tokens:
                color = (
                    "#ff7575"
                    if "Error" in t.name
                    else ("#fccb6f" if not t.is_mutable else "#8de3a1")
                )

                # STYLING: Array nodes get a massive border to look "stacked"
                node_params = {
                    "label": f"{t.name}[]" if t.is_collection else t.name,
                    "shape": "dot",
                    "color": color,
                    "size": 22 if t.is_collection else 15,
                    "borderWidth": 7 if t.is_collection else 1,
                    "color": {"border": "#ffffff", "background": color}
                    if t.is_collection
                    else color,
                }
                if t.is_input:
                    node_params.update({"x": -600, "fixed": True, "mass": 5})
                net.add_node(t.id, **node_params)
                added_tokens.add(t.id)

        # STYLING: Multi-edges (====>) are thicker and dashed
        for t in entry["consumed"]:
            edge_params = {"color": "#444444" if not t.is_mutable else "#8de3a1"}
            if t.is_collection:
                edge_params.update(
                    {"width": 7, "dashes": [10, 2]}
                )  # Double-arrow effect
            net.add_edge(t.id, f_id, **edge_params)

        for t in entry["produced"]:
            edge_params = {"color": "#00ffcc"}
            if t.is_collection:
                edge_params.update(
                    {"width": 7, "dashes": [10, 2]}
                )  # Double-arrow effect
            net.add_edge(f_id, t.id, **edge_params)

    net.set_options(
        '{"physics": {"barnesHut": {"gravitationalConstant": -15000, "avoidOverlap": 1}}}'
    )
    net.show(filename, notebook=False)


# --- 5. Run ---
ir_data = process_flow(my_flow)
generate_graph(ir_data)
