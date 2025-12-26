import itertools
from typing import List
from pydantic import BaseModel, Field
from pyvis.network import Network

_func_id_counter = itertools.count(1)


# --- 1. Models ---
class Type(BaseModel):
    name: str
    is_mutable: bool = True
    is_collection: bool = False

    def __hash__(self):
        return hash((self.name, self.is_mutable, self.is_collection))

    def __eq__(self, other):
        return (
            isinstance(other, Type)
            and self.name == other.name
            and self.is_mutable == other.is_mutable
            and self.is_collection == other.is_collection
        )


class TokenEdge(BaseModel):
    """Represents a data flow edge between functions"""

    name: str
    is_mutable: bool = True
    is_collection: bool = False
    origin_function_uid: int
    destination_function_uid: int

    @classmethod
    def from_type(cls, t: Type, origin: int, destination: int):
        return cls(
            name=t.name,
            is_mutable=t.is_mutable,
            is_collection=t.is_collection,
            origin_function_uid=origin,
            destination_function_uid=destination,
        )


class Data(Type):
    pass


class Error(Type):
    pass


class Function(BaseModel):
    name: str
    uid: int = Field(default_factory=lambda: next(_func_id_counter))
    consumes: List[Type] = []
    produces: List[Type] = []
    is_start: bool = False
    is_end: bool = False

    def __hash__(self):
        return hash(self.uid)


# --- 2. Definitions ---
InitialCommand = Data(name="InitialCommand")
PathToConfiguration = Data(name="PathToConfiguration")
SourceFile = Data(name="SourceFile", is_mutable=False, is_collection=True)
Article = Data(name="Article")
Html = Data(name="HTML")
Settings = Data(name="Settings", is_mutable=False)
SiteTemplates = Data(name="SiteTemplates", is_mutable=False)
FSError = Error(name="FileSystemError")

ProcessInitialCommand = Function(
    name="ProcessInitialCommand",
    consumes=[InitialCommand],
    produces=[Settings, PathToConfiguration],
)
LoadConfiguration = Function(
    name="LoadConfiguration",
    consumes=[PathToConfiguration],
    produces=[Settings],
)
LoadTemplates = Function(
    name="LoadTemplates",
    consumes=[Settings],
    produces=[SiteTemplates],
)
FindSourceFiles = Function(
    name="FindSourceFiles",
    consumes=[Settings],
    produces=[SourceFile, FSError],
)
ParseSource = Function(
    name="ParseSource",
    consumes=[SourceFile],
    produces=[Article, FSError],
)
RenderArticle = Function(
    name="RenderArticle",
    consumes=[Article, SiteTemplates, Settings],
    produces=[Html],
)
WriteHTML = Function(name="WriteHTML", consumes=[Html], produces=[FSError])

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
class TokenPool:
    """Manages available tokens with proper consumption semantics"""

    def __init__(self):
        # For mutable tokens: single (type, origin_uid)
        # For immutable tokens: list of all (type, origin_uid) producers
        self.mutable_tokens: List[tuple[Type, int]] = []
        self.immutable_tokens: dict[Type, List[int]] = {}  # type -> list of origin_uids

    def add(self, token_type: Type, origin_uid: int):
        """Add a new token to the pool"""
        if token_type.is_mutable:
            self.mutable_tokens.append((token_type, origin_uid))
        else:
            if token_type not in self.immutable_tokens:
                self.immutable_tokens[token_type] = []
            self.immutable_tokens[token_type].append(origin_uid)

    def consume(self, token_type: Type, consumer_uid: int) -> List[TokenEdge]:
        """
        Consume a token from the pool.
        - Mutable tokens: single edge, removed after consumption
        - Immutable tokens: multiple edges (one per producer), remain available
        """
        edges = []

        if token_type.is_mutable:
            # Find and remove the mutable token
            for i, (t, origin_uid) in enumerate(self.mutable_tokens):
                if t == token_type:
                    edge = TokenEdge.from_type(t, origin_uid, consumer_uid)
                    edges.append(edge)
                    self.mutable_tokens.pop(i)
                    break
        else:
            # Create edges from ALL producers of this immutable token
            if token_type in self.immutable_tokens:
                for origin_uid in self.immutable_tokens[token_type]:
                    edge = TokenEdge.from_type(token_type, origin_uid, consumer_uid)
                    edges.append(edge)

        return edges

    def get_unconsumed(self) -> List[tuple[Type, int]]:
        """Return all unconsumed tokens (mutable only and all immutable)"""
        unconsumed = self.mutable_tokens.copy()
        for token_type, origin_uids in self.immutable_tokens.items():
            for origin_uid in origin_uids:
                unconsumed.append((token_type, origin_uid))
        return unconsumed

    def __repr__(self):
        mutable = [t.name for t, _ in self.mutable_tokens]
        immutable = [
            f"{t.name}(x{len(origins)})" for t, origins in self.immutable_tokens.items()
        ]
        return f"TokenPool(mutable={mutable}, immutable={immutable})"


def process_flow(flow: List[Function]) -> tuple[List[Function], List[TokenEdge]]:
    """Process a flow and generate nodes and edges for visualization"""
    start_node = Function(name="Start", is_start=True)
    end_node = Function(name="End", is_end=True)

    nodes = [start_node]
    edges = []
    pool = TokenPool()

    # Initialize pool with first function's inputs from start node
    for token_type in flow[0].consumes:
        pool.add(token_type, start_node.uid)

    print(f"Initial pool: {pool}\n")

    # Process each function
    for func in flow:
        nodes.append(func)

        # Consume required inputs
        for consumed_type in func.consumes:
            consumed_edges = pool.consume(consumed_type, func.uid)
            if consumed_edges:
                edges.extend(consumed_edges)
                if not consumed_type.is_mutable:
                    print(
                        f"  ✓ {func.name} receives {consumed_type.name} from {len(consumed_edges)} producer(s)"
                    )
            else:
                print(
                    f"  ⚠️  Warning: {func.name} needs {consumed_type.name} but it's not available"
                )

        # Produce outputs
        for produced_type in func.produces:
            pool.add(produced_type, func.uid)

        print(f"{func.name}")
        print(f"  Pool after: {pool}\n")

    # Connect unconsumed tokens to end node
    for token_type, origin_uid in pool.get_unconsumed():
        edge = TokenEdge.from_type(token_type, origin_uid, end_node.uid)
        edges.append(edge)

    nodes.append(end_node)

    # Print edge summary
    print("\n=== Edges ===")
    for edge in edges:
        origin_name = next(n.name for n in nodes if n.uid == edge.origin_function_uid)
        dest_name = next(
            n.name for n in nodes if n.uid == edge.destination_function_uid
        )
        mutability = "immutable" if not edge.is_mutable else "mutable"
        print(f"{origin_name} -> {dest_name}: {edge.name} ({mutability})")

    return nodes, edges


# --- 4. Visualizer ---
def generate_graph(
    nodes: List[Function], edges: List[TokenEdge], filename="architecture.html"
):
    net = Network(
        height="900px",
        width="100%",
        bgcolor="#121212",
        font_color="white",  # type: ignore
        directed=True,
    )

    # Configure physics for longer edges
    net.set_options("""
    {
      "physics": {
        "enabled": true,
        "barnesHut": {
          "gravitationalConstant": -8000,
          "centralGravity": 0.3,
          "springLength": 200,
          "springConstant": 0.04,
          "damping": 0.09,
          "avoidOverlap": 0.5
        },
        "minVelocity": 0.75,
        "solver": "barnesHut"
      }
    }
    """)

    for node in nodes:
        color = "#8784DA" if node.is_start or node.is_end else "#1347B8"
        net.add_node(
            node.uid,
            label=node.name,
            shape="dot",
            color=color,
            size=15,
        )

    for edge in edges:
        label = edge.name + ("[]" if edge.is_collection else "")
        color = "#00ffcc" if edge.is_mutable else "#888888"
        width = 10.0 if edge.is_collection else 1.5
        dashes = [8, 4] if edge.is_collection else False

        net.add_edge(
            edge.origin_function_uid,
            edge.destination_function_uid,
            label=label,
            color=color,
            width=width,
            dashes=dashes,
        )

    net.show(filename, notebook=False)


# --- 5. Run ---
if __name__ == "__main__":
    nodes, edges = process_flow(my_flow)
    generate_graph(nodes, edges)
