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


class Output(BaseModel):
    data_type: Type
    is_collection: bool = False


class Function(BaseModel):
    name: str
    consumes: List[Type]
    produces: List[Output]


class TokenInstance:
    def __init__(
        self, data_type: Type, is_collection: bool = False, is_input: bool = False
    ):
        self.id = f"node_{uuid.uuid4().hex[:8]}"
        self.name = data_type.name
        self.is_mutable = data_type.is_mutable
        self.is_collection = is_collection
        self.is_input = is_input


# --- 2. Definitions ---
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


# --- 3. Logic Engine ---
def process_flow(flow: List[Function]) -> List[Dict[str, Any]]:
    pool = [TokenInstance(t, is_input=True) for t in flow[0].consumes]
    ir = []
    for i, func in enumerate(flow, 1):
        consumed, is_iterative = [], False
        for req in func.consumes:
            token = next((t for t in pool if t.name == req.name), None)
            if not token:
                raise ValueError(f"Missing {req.name}")
            if token.is_collection:
                is_iterative = True
            consumed.append(token)
            if token.is_mutable:
                pool.remove(token)
        produced = []
        for out in func.produces:
            existing = next((t for t in pool if t.name == out.data_type.name), None)
            if not out.data_type.is_mutable and existing:
                produced.append(existing)
            else:
                new_t = TokenInstance(
                    out.data_type, is_collection=out.is_collection or is_iterative
                )
                pool.append(new_t)
                produced.append(new_t)
        ir.append(
            {
                "step": i,
                "function": func.name,
                "consumed": consumed,
                "produced": produced,
                "is_iterative": is_iterative,
            }
        )
    return ir


# --- 4. Visualizer ---
def generate_graph(ir: List[Dict[str, Any]], filename="architecture.html"):
    net = Network(
        height="900px",
        width="100%",
        bgcolor="#121212",
        font_color="white",
        directed=True,
        layout=True,
    )

    # all_produced = {t.id for entry in ir for t in entry["produced"]}
    # all_consumed = {t.id for entry in ir for t in entry["consumed"]}
    # terminal_ids = all_produced - all_consumed
    added_tokens = set()

    for entry in ir:
        f_id = f"f_{entry['step']}"
        net.add_node(
            f_id,
            label=entry["function"],
            shape="diamond",
            color="#6fb1fc",
            size=15,
            borderWidth=2 if entry["is_iterative"] else 1,
        )

        for t in entry["consumed"] + entry["produced"]:
            if t.id not in added_tokens:
                base_color = "#8de3a1"  # Green
                if "Error" in t.name:
                    base_color = "#ff7575"  # Red
                elif not t.is_mutable:
                    base_color = "#fccb6f"  # Yellow

                node_params = {
                    "label": f"{t.name}[]" if t.is_collection else t.name,
                    "shape": "dot",
                    "color": base_color,
                    "size": 12,
                    "mass": 1,
                }

                if t.is_collection:
                    node_params.update(
                        {
                            "size": 15,
                            "borderWidth": 3,
                            "color": {"border": "#ffffff", "background": base_color},
                        }
                    )

                # --- START/END NODES POSITIONING (COMMENTED OUT) ---
                # if t.is_input:
                #     node_params.update({"x": -600, "fixed": True, "mass": 2})
                # if t.id in terminal_ids:
                #     node_params.update({"x": 600, "fixed": True, "mass": 2})

                net.add_node(t.id, **node_params)
                added_tokens.add(t.id)

        for t in entry["consumed"]:
            e_color = "#444444" if not t.is_mutable else "#8de3a1"
            width = 2.5 if t.is_collection else 1
            dashes = [8, 4] if t.is_collection else False
            net.add_edge(t.id, f_id, color=e_color, width=width, dashes=dashes)

        for t in entry["produced"]:
            width = 2.5 if t.is_collection else 1
            dashes = [8, 4] if t.is_collection else False
            net.add_edge(f_id, t.id, color="#00ffcc", width=width, dashes=dashes)

    net.set_options("""
    var options = {
      "physics": {
        "barnesHut": { "gravitationalConstant": -6000, "springLength": 120, "avoidOverlap": 1 },
        "minVelocity": 0.75
      }
    }
    """)
    net.show(filename, notebook=False)


# --- 5. Run ---
ir_data = process_flow(my_flow)
generate_graph(ir_data)
