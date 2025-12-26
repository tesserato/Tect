import itertools
from enum import Enum
from typing import List, Optional, Tuple, Dict

from pydantic import BaseModel
from pyvis.network import Network

# --- Global Configuration & Counters ---
# Used to ensure every function/node in the graph has a unique identifier
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

    def __hash__(self):
        return hash((self.name, self.is_mutable))

    def __eq__(self, other):
        return (
            isinstance(other, Type)
            and self.name == other.name
            and self.is_mutable == other.is_mutable
        )


class Data(Type):
    """Specific Type subclass for standard data payloads."""

    pass


class Error(Type):
    """Specific Type subclass for error/exception payloads."""

    pass


# --- 2. Data Flow Models ---


class Token(BaseModel):
    """
    The 'Edge' in the graph. Represents a piece of data moving between functions.
    Contains metadata about the data type, collection status, and flow connectivity.
    """

    name: str
    is_mutable: bool = True
    is_collection: bool = False
    origin_function_uid: Optional[int] = None
    destination_function_uid: Optional[int] = None

    @classmethod
    def from_type(
        cls,
        t: Type,
        origin: Optional[int] = None,
        destination: Optional[int] = None,
        is_collection: bool = False,
    ):
        """Helper to create a Token from a Type definition."""
        return cls(
            name=t.name,
            is_mutable=t.is_mutable,
            is_collection=is_collection,
            origin_function_uid=origin,
            destination_function_uid=destination,
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
    is_start: bool = False
    is_end: bool = False
    is_error: bool = False

    def __hash__(self):
        return hash(self.uid)


# --- 3. Component Factory ---


def generate_function(
    name: str,
    consumes: List[Tuple[Type, Cardinality]] = [],
    produces: List[Tuple[Type, Cardinality]] = [],
    is_start: bool = False,
    is_end: bool = False,
    is_error: bool = False,
) -> Function:
    """
    Factory function to streamline the creation of Functions.
    Converts Type/Cardinality tuples into Token objects and assigns unique UIDs.
    """
    consumes_as_tokens = [
        Token.from_type(t, is_collection=(c == Cardinality.MANY)) for t, c in consumes
    ]
    produces_as_tokens = [
        Token.from_type(t, is_collection=(c == Cardinality.MANY)) for t, c in produces
    ]

    return Function(
        name=name,
        uid=next(_func_id_counter),
        consumes=consumes_as_tokens,
        produces=produces_as_tokens,
        is_start=is_start,
        is_end=is_end,
        is_error=is_error,
    )


# --- 4. Logic Engine (Simulation) ---


class TokenPool:
    """
    Simulates the 'State' of the application during runtime.
    Manages available tokens and handles the logic for mutable vs. immutable consumption.
    """

    def __init__(self):
        self.available_tokens: List[Token] = []
        self.consumed_tokens: List[Token] = []

    def add(self, token: Token, origin_uid: int):
        """Registers a produced token into the environment."""
        new_token = token.model_copy()
        new_token.origin_function_uid = origin_uid
        self.available_tokens.append(new_token)

    def consume(self, requirement: Token, consumer_uid: int) -> List[Token]:
        """
        Attempts to satisfy a function's requirement from the pool.
        - Immutable tokens: Create an edge but remain in pool for others.
        - Mutable tokens: Create an edge and are removed from pool.
        """
        edges = []
        # Find matches based on token name
        matches = [t for t in self.available_tokens if t.name == requirement.name]

        for match in matches:
            # Create a specific flow edge for the graph
            edge = match.model_copy()
            edge.destination_function_uid = consumer_uid
            edges.append(edge)

            self.consumed_tokens.append(match)

            if match.is_mutable:
                self.available_tokens.remove(match)
                break  # Mutable requirements are satisfied by the first available match

        return edges

    def get_unconsumed(self) -> List[Token]:
        """Returns tokens that were produced but never used by another function."""
        return [t for t in self.available_tokens if t not in self.consumed_tokens]


def process_flow(flow: List[Function]) -> Tuple[List[Function], List[Token]]:
    """
    The orchestrator. It simulates the flow of data through a sequence of functions,
    connecting producers to consumers and handling terminal (Start/End/Error) nodes.
    """
    start_node = generate_function(name="Start", is_start=True)
    end_node = generate_function(name="End", is_end=True)

    nodes = [start_node]
    all_edges = []
    pool = TokenPool()

    # Seed the initial pool with requirements for the first function, sourced from Start
    if flow:
        for req in flow[0].consumes:
            pool.add(req, start_node.uid)

    # Main Simulation Loop
    for func in flow:
        nodes.append(func)

        # 1. Satisfy consumption requirements
        for req in func.consumes:
            new_edges = pool.consume(req, func.uid)
            all_edges.extend(new_edges)

        # 2. Add produced outputs to the pool
        for prod in func.produces:
            pool.add(prod, func.uid)

    # Finalization: Handle unconsumed data and errors
    error_nodes: Dict[str, Function] = {}
    for leftover in pool.get_unconsumed():
        # Heuristic: If it's a Type/subclass named 'Error' or explicitly an Error class
        if "Error" in leftover.name:
            if leftover.name not in error_nodes:
                err_node = generate_function(
                    name=leftover.name, is_end=True, is_error=True
                )
                error_nodes[leftover.name] = err_node
                nodes.append(err_node)

            edge = leftover.model_copy()
            edge.destination_function_uid = error_nodes[leftover.name].uid
            all_edges.append(edge)
        else:
            # Standard unconsumed data flows to the 'End' terminal
            edge = leftover.model_copy()
            edge.destination_function_uid = end_node.uid
            all_edges.append(edge)

    nodes.append(end_node)
    return nodes, all_edges


# --- 5. Visualization Engine ---


def generate_graph(
    nodes: List[Function], edges: List[Token], filename="architecture.html"
):
    """Generates an interactive HTML visualization of the architecture."""
    net = Network(
        height="900px",
        width="100%",
        bgcolor="#121212",
        font_color="white",  # type: ignore
        directed=True,
    )

    # 1. Create Nodes
    for node in nodes:
        color = "#2921FF"  # Default Function color
        if node.is_error:
            color = "#FF4444"
        elif node.is_start or node.is_end:
            color = "#00CCFF"

        net.add_node(
            node.uid,
            label=node.name,
            shape="dot",
            color=color,
            size=20 if node.is_start else 15,
        )

    # 2. Create Edges
    for edge in edges:
        # Validate that both ends of the edge exist
        if (
            edge.origin_function_uid is not None
            and edge.destination_function_uid is not None
        ):
            label = edge.name + ("[]" if edge.is_collection else "")
            color = "#00ffcc" if edge.is_mutable else "#888888"

            net.add_edge(
                edge.origin_function_uid,
                edge.destination_function_uid,
                label=label,
                color=color,
                width=4 if edge.is_collection else 1.5,
                dashes=edge.is_collection,
                font={"size": 10, "color": "#ffffff"},
            )

    net.show_buttons(filter_=["physics"])
    net.show(filename, notebook=False)
    print(f"Graph generated successfully: {filename}")


# --- 6. Execution (Example Static Site Generator) ---

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

    # Define the Processing Pipeline
    pipeline = [
        generate_function(
            "ProcessCLI",
            consumes=[(InitialCommand, Cardinality.ONE)],
            produces=[(Settings, Cardinality.ONE), (PathToConfig, Cardinality.ONE)],
        ),
        generate_function(
            "LoadConfig",
            consumes=[(PathToConfig, Cardinality.ONE)],
            produces=[(Settings, Cardinality.ONE)],
        ),
        generate_function(
            "LoadTemplates",
            consumes=[(Settings, Cardinality.ONE)],
            produces=[(Templates, Cardinality.ONE)],
        ),
        generate_function(
            "ScanFS",
            consumes=[(Settings, Cardinality.ONE)],
            produces=[(SourceFile, Cardinality.MANY), (FSError, Cardinality.MANY)],
        ),
        generate_function(
            "ParseMarkdown",
            consumes=[(SourceFile, Cardinality.ONE)],
            produces=[(Article, Cardinality.ONE), (FSError, Cardinality.ONE)],
        ),
        generate_function(
            "RenderHTML",
            consumes=[
                (Article, Cardinality.ONE),
                (Templates, Cardinality.ONE),
                (Settings, Cardinality.ONE),
            ],
            produces=[(Html, Cardinality.ONE)],
        ),
        generate_function(
            "WriteToDisk",
            consumes=[(Html, Cardinality.ONE)],
            produces=[(Success, Cardinality.ONE), (FSError, Cardinality.ONE)],
        ),
    ]

    # Process and Visualize
    nodes, edges = process_flow(pipeline)
    generate_graph(nodes, edges)
