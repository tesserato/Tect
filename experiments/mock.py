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
    Immutable types can be consumed by multiple functions (e.g., Configuration).
    Mutable types are consumed once and removed from the flow (e.g., a File Handle).
    """

    name: str
    is_mutable: bool = True


class Data(Type):
    """Specific Type subclass for standard data payloads."""

    pass


class Error(Type):
    """Specific Type subclass for error/exception payloads."""

    pass


# --- Core Models ---


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
    Functions declare what they consume from the environment and what they produce.
    """

    name: str
    uid: int
    consumes: List[Token]
    produces: List[Token]
    is_artificial_graph_start: bool = False
    is_artificial_graph_end: bool = False
    is_artificial_error_termination: bool = False


# --- Factory ---


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


# --- Logic Engine ---


class TokenPool:
    """
    Simulates the 'State' of the application during runtime.
    Manages available tokens and handles logic for mutable vs. immutable consumption.
    """

    def __init__(self):
        self.available: List[Token] = []
        self.consumed: List[Token] = []

    def add(self, token: Token, origin_uid: int, force_collection: bool = False):
        """Adds a produced token to the pool, optionally forcing it to be a collection."""
        t = token.model_copy()
        t.origin_function_uid = origin_uid
        if force_collection:
            t.is_collection = True
        self.available.append(t)

    def consume_requirement(
        self, req: Token, consumer_uid: int
    ) -> Tuple[List[Token], bool]:
        """
        Attempts to satisfy a function requirement.
        Returns (satisfied_edges, triggered_expansion).
        'triggered_expansion' is true if we consumed a MANY as a ONE (Fan-out).
        """
        edges = []
        triggered_expansion = False

        # Find matches for this type
        matches = [t for t in self.available if t.name == req.name]

        for match in matches:
            edge = match.model_copy()
            edge.destination_function_uid = consumer_uid

            # --- FAN-OUT LOGIC ---
            # If the source is a collection, but the consumer wants ONE,
            # this function is now operating in an "expanded" context.
            if match.is_collection and not req.is_collection:
                triggered_expansion = True

            edges.append(edge)
            self.consumed.append(match)

            if match.is_mutable:
                self.available.remove(match)
                break  # Consumed the resource

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

        # Per-function expansion state
        func_is_expanded = False

        # 1. Consume inputs
        for req in func.consumes:
            edges, expanded = pool.consume_requirement(req, func.uid)
            all_edges.extend(edges)
            if expanded:
                func_is_expanded = True

        # 2. Produce outputs (inherit expansion state)
        for prod in func.produces:
            pool.add(prod, func.uid, force_collection=func_is_expanded)

    # Route leftovers to terminal nodes
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


# --- Visualizer ---


def generate_graph(
    nodes: List[Function], edges: List[Token], filename="architecture.html"
):
    net = Network(
        height="900px",
        width="100%",
        bgcolor="#0b0e14",
        font_color="#e0e0e0",  # type: ignore
        directed=True,
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
        # "edges": {
        #     "smooth": {
        #         "type": "cubicBezier",
        #         "forceDirection": "vertical",
        #         "roundness": 0.4,
        #     },
        #     "font": {
        #         "strokeWidth": 0,
        #         "size": 11,
        #         "color": "#ffffff",
        #         "align": "middle",
        #     },
        # },
        # "nodes": {"font": {"face": "Tahoma", "size": 16}},
    }

    for n in nodes:
        color = "#1d4ed8"
        if n.is_artificial_error_termination:
            color = "#dc2626"
        elif n.is_artificial_graph_start or n.is_artificial_graph_end:
            color = "#059669"

        net.add_node(
            n.uid,
            label=f" {n.name} ",
            shape="box",
            color={"background": color, "border": "#ffffff"},  # type: ignore
            borderWidth=1,
            margin=10,
        )

    for e in edges:
        if e.origin_function_uid is not None and e.destination_function_uid is not None:
            is_many = e.is_collection
            label = e.name + ("[]" if is_many else "")

            net.add_edge(
                e.origin_function_uid,
                e.destination_function_uid,
                label=label,
                color="#818cf8" if is_many else "#94a3b8",
                width=4 if is_many else 1.5,
                dashes=[12, 4] if is_many else False,
                arrowStrikethrough=False,
            )

    net.set_options(json.dumps(options))
    net.show(filename, notebook=False)
    print(f"Graph generated: {filename}")


def save_to_json(
    nodes: List[Function], edges: List[Token], filename: str = "architecture.json"
):
    """Serializes the processed graph nodes and edges to a JSON file."""
    export_data = {
        "nodes": [n.model_dump() for n in nodes],
        "edges": [e.model_dump() for e in edges],
    }

    with open(filename, "w", encoding="utf-8") as f:
        json.dump(export_data, f, indent=4)

    print(f"Architecture exported to {filename}")


# --- Execution ---

if __name__ == "__main__":
    # Define Domain Types
    InitialCommand = Data(name="InitialCommand")
    PathToConfig = Data(name="PathToConfig")
    SourceFile = Data(name="SourceFile", is_mutable=False)
    Article = Data(name="Article")
    Html = Data(name="HTML")
    Settings = Data(name="Settings", is_mutable=False)
    Templates = Data(name="Templates", is_mutable=False)
    FSError = Error(name="FileSystemError")
    Success = Data(name="SuccessReport", is_mutable=False)

    # Define the Pipeline
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
        # 1. FAN-OUT: Produces MANY SourceFiles
        generate_function(
            "ScanFS",
            [(Settings, Cardinality.ONE)],
            [(SourceFile, Cardinality.MANY), (FSError, Cardinality.MANY)],
        ),
        # 2. PROPAGATION: Receives MANY (as ONE) -> Article becomes MANY automatically
        generate_function(
            "ParseMarkdown",
            [(SourceFile, Cardinality.ONE)],
            [(Article, Cardinality.ONE), (FSError, Cardinality.ONE)],
        ),
        # 3. PROPAGATION: HTML remains MANY
        generate_function(
            "RenderHTML",
            [
                (Article, Cardinality.ONE),
                (Templates, Cardinality.ONE),
                (Settings, Cardinality.ONE),
            ],
            [(Html, Cardinality.ONE)],
        ),
        # 4. FAN-IN: Consumes MANY HTML -> SuccessReport returns to ONE
        generate_function(
            "WriteToDisk",
            [(Html, Cardinality.MANY)],
            [(Success, Cardinality.ONE), (FSError, Cardinality.MANY)],
        ),
    ]

    nodes, edges = process_flow(pipeline)
    save_to_json(nodes, edges)
    generate_graph(nodes, edges)
