import itertools
import json
from enum import Enum
from typing import List, Optional, Tuple

from pydantic import BaseModel
from pyvis.network import Network

# --- Global Configuration ---
_func_id_counter = itertools.count(1)


# --- 1. Enumerations & Base Types ---


class Cardinality(Enum):
    """Defines whether a function handles a single item or a collection."""

    ONE = "1"
    MANY = "*"


class Type(BaseModel):
    """
    Represents the 'Schema' of data.
    Immutable types can be consumed by multiple functions.
    Mutable types are consumed once and removed from the flow.
    """

    name: str
    is_mutable: bool = True


class Data(Type):
    """Specific Type subclass for standard data payloads."""

    pass


class Error(Type):
    """Specific Type subclass for error/exception payloads."""

    pass


# --- 2. Core Models ---


class Token(BaseModel):
    """Represents data moving through the system."""

    name: str
    is_mutable: bool = True
    is_collection: bool = False
    origin_function_uid: Optional[int] = None
    destination_function_uid: Optional[int] = None

    @classmethod
    def from_type(
        cls, t: Type, origin: Optional[int] = None, is_collection: bool = False
    ):
        return cls(
            name=t.name,
            is_mutable=t.is_mutable,
            is_collection=is_collection,
            origin_function_uid=origin,
        )


class Function(BaseModel):
    """
    The 'Node' in the graph. Represents a processing unit.
    """

    name: str
    uid: int
    consumes: List[Token]
    produces: List[Token]
    is_artificial_graph_start: bool = False
    is_artificial_graph_end: bool = False
    is_artificial_error_termination: bool = False


# --- 3. Factory ---


def generate_function(
    name: str,
    consumes: List[Tuple[Type, Cardinality]] = [],
    produces: List[Tuple[Type, Cardinality]] = [],
    is_start=False,
    is_end=False,
    is_error=False,
) -> Function:
    c_tokens = [
        Token.from_type(t, is_collection=(c == Cardinality.MANY)) for t, c in consumes
    ]
    p_tokens = [
        Token.from_type(t, is_collection=(c == Cardinality.MANY)) for t, c in produces
    ]
    return Function(
        name=name,
        uid=next(_func_id_counter),
        consumes=c_tokens,
        produces=p_tokens,
        is_artificial_graph_start=is_start,
        is_artificial_graph_end=is_end,
        is_artificial_error_termination=is_error,
    )


# --- 4. Logic Engine ---


class TokenPool:
    """
    Manages available tokens. Handles logic for mutable vs. immutable consumption.
    """

    def __init__(self):
        self.available: List[Token] = []
        self.consumed: List[Token] = []

    def add(self, token: Token, origin_uid: int, force_collection: bool = False):
        t = token.model_copy()
        t.origin_function_uid = origin_uid
        if force_collection:
            t.is_collection = True
        self.available.append(t)

    def consume_requirement(
        self, req: Token, consumer_uid: int
    ) -> Tuple[List[Token], bool]:
        """Returns (satisfied_edges, triggered_expansion)."""
        edges = []
        triggered_expansion = False
        matches = [t for t in self.available if t.name == req.name]

        for match in matches:
            edge = match.model_copy()
            edge.destination_function_uid = consumer_uid

            # Expansion Logic: Collection consumed as a Single item
            if match.is_collection and not req.is_collection:
                triggered_expansion = True

            edges.append(edge)
            self.consumed.append(match)

            if match.is_mutable:
                self.available.remove(match)
                break

        return edges, triggered_expansion


def process_flow(flow: List[Function]) -> Tuple[List[Function], List[Token]]:
    start_node = generate_function("Start", is_start=True)
    end_node = generate_function("End", is_end=True)
    nodes, all_edges, pool = [start_node], [], TokenPool()

    # Initial seeding from Start node
    if flow:
        for req in flow[0].consumes:
            pool.add(req, start_node.uid)

    for func in flow:
        nodes.append(func)
        func_is_expanded = False

        for req in func.consumes:
            edges, expanded = pool.consume_requirement(req, func.uid)
            all_edges.extend(edges)
            if expanded:
                func_is_expanded = True

        for prod in func.produces:
            pool.add(prod, func.uid, force_collection=func_is_expanded)

    # Handle Terminal/Error Routing
    error_nodes = {}
    for leftover in pool.available:
        if leftover in pool.consumed:
            continue
        target = end_node
        if "Error" in leftover.name:
            if leftover.name not in error_nodes:
                error_nodes[leftover.name] = generate_function(
                    leftover.name, is_end=True, is_error=True
                )
                nodes.append(error_nodes[leftover.name])
            target = error_nodes[leftover.name]

        edge = leftover.model_copy()
        edge.destination_function_uid = target.uid
        all_edges.append(edge)

    nodes.append(end_node)
    return nodes, all_edges


# --- 5. Visualizer & Persistence ---


def save_to_json(
    nodes: List[Function], edges: List[Token], filename: str = "architecture.json"
):
    """Serializes the graph to JSON."""
    export_data = {
        "nodes": [n.model_dump() for n in nodes],
        "edges": [e.model_dump() for e in edges],
    }
    with open(filename, "w", encoding="utf-8") as f:
        json.dump(export_data, f, indent=4)
    print(f"Architecture exported to {filename}")


def generate_graph(json_input_file: str, html_output_file: str = "architecture.html"):
    """
    Loads JSON and generates a color-coded Pyvis visualization.
    """
    with open(json_input_file, "r", encoding="utf-8") as f:
        data = json.load(f)

    net = Network(
        height="900px",
        width="100%",
        bgcolor="#0b0e14",
        font_color="#e0e0e0",  # type: ignore
        directed=True,
    )

    # Add Nodes
    for n in data.get("nodes", []):
        # FIX: Align dictionary keys with model field names
        if n.get("is_artificial_error_termination"):
            color = "#dc2626"  # Red
        elif n.get("is_artificial_graph_start") or n.get("is_artificial_graph_end"):
            color = "#059669"  # Emerald
        else:
            color = "#1d4ed8"  # Blue

        net.add_node(
            n["uid"],
            label=f" {n['name']} ",
            shape="box",
            color={"background": color, "border": "#ffffff"}, # type: ignore
            borderWidth=1,
            margin=10,
        )

    # Add Edges
    for e in data.get("edges", []):
        u, v = e.get("origin_function_uid"), e.get("destination_function_uid")
        if u is not None and v is not None:
            is_many = e.get("is_collection", False)
            net.add_edge(
                u,
                v,
                label=e["name"] + ("[]" if is_many else ""),
                color="#818cf8" if is_many else "#94a3b8",
                width=4 if is_many else 1.5,
                dashes=[12, 4] if is_many else False,
            )

    options = {
"physics": {
            "forceAtlas2Based": {
                "theta": 0.1,
                "gravitationalConstant": -105,
                "springLength": 5,
                "damping": 1,
                "avoidOverlap": 1,
            },
            "minVelocity": 0.75,
            "solver": "forceAtlas2Based",
            "timestep": 0.01,
        },
    }
    net.set_options(json.dumps(options))
    net.show(html_output_file, notebook=False)
    print(f"Graph generated: {html_output_file}")


# --- 6. Execution ---


if __name__ == "__main__":
    # Define Types
    InitialCommand = Data(name="InitialCommand")
    PathToConfig = Data(name="PathToConfig")
    SourceFile = Data(name="SourceFile", is_mutable=False)
    Article = Data(name="Article")
    Html = Data(name="HTML")
    Settings = Data(name="Settings", is_mutable=False)
    Templates = Data(name="Templates", is_mutable=False)
    FSError = Error(name="FileSystemError")
    Success = Data(name="SuccessReport", is_mutable=False)

    # Define Pipeline
    pipeline = [
        generate_function(
            "ProcessCLI",
            [(InitialCommand, Cardinality.ONE)],
            [(Settings, Cardinality.ONE), (PathToConfig, Cardinality.ONE)],
        ),
        generate_function(
            "LoadConfig",
            [(PathToConfig, Cardinality.ONE)],
            [(Settings, Cardinality.ONE)],
        ),
        generate_function(
            "LoadTemplates",
            [(Settings, Cardinality.ONE)],
            [(Templates, Cardinality.ONE)],
        ),
        generate_function(
            "ScanFS",
            [(Settings, Cardinality.ONE)],
            [(SourceFile, Cardinality.MANY), (FSError, Cardinality.MANY)],
        ),
        generate_function(
            "ParseMarkdown",
            [(SourceFile, Cardinality.ONE)],
            [(Article, Cardinality.ONE), (FSError, Cardinality.ONE)],
        ),
        generate_function(
            "RenderHTML",
            [
                (Article, Cardinality.ONE),
                (Templates, Cardinality.ONE),
                (Settings, Cardinality.ONE),
            ],
            [(Html, Cardinality.ONE)],
        ),
        generate_function(
            "WriteToDisk",
            [(Html, Cardinality.MANY)],
            [(Success, Cardinality.ONE), (FSError, Cardinality.MANY)],
        ),
    ]

    # nodes, edges = process_flow(pipeline)
    # save_to_json(nodes, edges, "architecture.json")
    generate_graph("architecture.json", "architecture.html")
