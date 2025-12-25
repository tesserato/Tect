from pydantic import BaseModel
from typing import List


class Type(BaseModel):
    name: str
    is_mutable: bool = True  # True = Consumed (Var), False = Persistent (Const)

    def __hash__(self):
        return hash((self.name, self.is_mutable))

    def __eq__(self, other):
        if not isinstance(other, Type):
            return False
        return self.name == other.name and self.is_mutable == other.is_mutable


class Function(BaseModel):
    name: str
    consumes: List[Type]
    produces: List[Type]


# --- 1. Define Data Types ---
InitialCommand = Type(name="InitialCommand")
PathToConfiguration = Type(name="PathToConfiguration")
SourceFile = Type(name="SourceFile")
Article = Type(name="Article")
HTML = Type(name="HTML")

# Immutable (Persistent State)
Settings = Type(name="Settings", is_mutable=False)
SiteTemplates = Type(name="SiteTemplates", is_mutable=False)

# Errors (Mutable - we want to track every single error that happens)
FSError = Type(name="FileSystemError")

# --- 2. Define Functions ---
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

# ErrorHandler = Function(name="ErrorHandler", consumes=[FSError], produces=[])


# We define the sequence of functions
my_flow = [
    ProcessInitialCommand,
    LoadConfiguration,
    LoadTemplates,
    FindSourceFiles,
    ParseSource,
    RenderArticle,
    WriteHTML,
    # ErrorHandler,  # Handles error from ParseSource
    # ErrorHandler,  # Handles error from WriteHTML
]


# --- 3. The Updated Validator Engine ---
def validate_architecture(initial_pool: List[Type], flow: List[Function]):
    pool = initial_pool.copy()

    print("-" * 60)
    print(f"STARTING POOL: {[t.name for t in pool]}")
    print("-" * 60)

    for i, func in enumerate(flow, 1):
        print(f"\nSTEP {i}: {func.name}")

        # 1. Consumption Logic
        for req in func.consumes:
            if req not in pool:
                raise ValueError(
                    f"❌ ERROR: '{func.name}' needs '{req.name}', but it's not in the pool."
                )

            if req.is_mutable:
                pool.remove(req)
                print(f"  [-] Consumed: {req.name}")
            else:
                print(f"  [∞] Accessed: {req.name} (Persists)")

        # 2. Idempotent Production Logic
        for p in func.produces:
            if not p.is_mutable and p in pool:
                # If it's immutable and we already have it, don't duplicate
                print(f"  [~] Persistent: {p.name} is already in the pool (no change)")
            else:
                # If it's mutable (like an error or data) OR a new immutable, add it
                pool.append(p)
                print(f"  [+] Produced: {p.name}")

        # 3. State Output
        print(f"  >> CURRENT POOL: {[t.name for t in pool]}")

    print("\n" + "-" * 60)
    # Final Validation
    unhandled_errors = [t for t in pool if "Error" in t.name]
    if unhandled_errors:
        print(
            f"⚠️  CRITICAL FAILURE: {len(unhandled_errors)} unhandled errors remain: {[e.name for e in unhandled_errors]}"
        )
    else:
        print("✅ SUCCESS: Flow complete. No unhandled errors.")
    print("-" * 60)


# --- 4. Execution ---


my_initial_data = my_flow[0].consumes

validate_architecture(my_initial_data, my_flow)


# Graph Visualization

import uuid
from pyvis.network import Network
from typing import List
from pydantic import BaseModel


# --- 1. Base Classes ---
class Type(BaseModel):
    name: str
    is_mutable: bool = True

    def __hash__(self):
        return hash((self.name, self.is_mutable))

    def __eq__(self, other):
        if not isinstance(other, Type):
            return False
        return self.name == other.name and self.is_mutable == other.is_mutable


class Function(BaseModel):
    name: str
    consumes: List[Type]
    produces: List[Type]


class TokenInstance:
    def __init__(self, data_type):
        self.id = f"node_{str(uuid.uuid4())[:8]}"
        self.name = data_type.name
        self.is_mutable = data_type.is_mutable


# --- 2. Architecture Trace Engine ---
def validate_and_trace(initial_types: List[Type], flow: List[Function]):
    pool = [TokenInstance(t) for t in initial_types]
    trace_events = []

    for func in flow:
        consumed_tokens = []
        for req_type in func.consumes:
            token = next((t for t in pool if t.name == req_type.name), None)
            if not token:
                raise ValueError(f"❌ Missing '{req_type.name}' for '{func.name}'")

            consumed_tokens.append(token)
            if token.is_mutable:
                pool.remove(token)

        produced_tokens = []
        for prod_type in func.produces:
            existing = next((t for t in pool if t.name == prod_type.name), None)
            if not prod_type.is_mutable and existing:
                produced_tokens.append(existing)
            else:
                new_t = TokenInstance(prod_type)
                pool.append(new_t)
                produced_tokens.append(new_t)

        trace_events.append((func.name, consumed_tokens, produced_tokens))

    return trace_events


# --- 3. The "Loose" Visualizer ---
def generate_loose_graph(trace_events, filename="architecture_loose.html"):
    # We use a dark background to make the "constellation" pop
    net = Network(
        height="900px",
        width="100%",
        bgcolor="#121212",
        font_color="white",
        directed=True,
    )
    added_node_ids = set()

    for i, (func_name, consumed, produced) in enumerate(trace_events):
        func_node_id = f"func_{i}"

        # Functions are the "logic anchors" (Diamonds)
        net.add_node(
            func_node_id,
            label=func_name,
            shape="diamond",
            color="#6fb1fc",
            size=25,
            mass=2,
        )

        # Data and Errors are the "resources" (Simple Dots)
        for t in consumed + produced:
            if t.id not in added_node_ids:
                # Color code purely based on type (Error=Red, Immutable=Yellow, Mutable=Green)
                color = "#8de3a1"  # Default Green
                if "Error" in t.name:
                    color = "#ff7575"
                elif not t.is_mutable:
                    color = "#fccb6f"

                # Simplified: No shapes like square/star, no "START/FINAL" text
                net.add_node(t.id, label=t.name, shape="dot", color=color, size=15)
                added_node_ids.add(t.id)

        # Create the springs
        for t in consumed:
            net.add_edge(t.id, func_node_id, color="#444444", width=1)
        for t in produced:
            net.add_edge(func_node_id, t.id, color="#00ffcc", width=2)

    # Use BarnesHut physics for a loose, organic grouping
    net.set_options("""
    var options = {
      "physics": {
        "barnesHut": {
          "gravitationalConstant": -10000,
          "centralGravity": 0.2,
          "springLength": 150,
          "springStrength": 0.05,
          "damping": 0.09,
          "avoidOverlap": 0.5
        },
        "minVelocity": 0.75
      },
      "edges": {
        "smooth": {
          "type": "continuous",
          "forceDirection": "none"
        }
      }
    }
    """)

    net.show(filename, notebook=False)
    print(f"Loose graph generated: {filename}")


# --- 4. Setup & Run ---
InitialCommand = Type(name="InitialCommand")
PathToConfiguration = Type(name="PathToConfiguration")
SourceFile = Type(name="SourceFile")
Article = Type(name="Article")
HTML = Type(name="HTML")
Settings = Type(name="Settings", is_mutable=False)
SiteTemplates = Type(name="SiteTemplates", is_mutable=False)
FSError = Type(name="FileSystemError")

my_flow = [
    Function(
        name="ProcessInitialCommand",
        consumes=[InitialCommand],
        produces=[Settings, PathToConfiguration],
    ),
    Function(
        name="LoadConfiguration", consumes=[PathToConfiguration], produces=[Settings]
    ),
    Function(name="LoadTemplates", consumes=[Settings], produces=[SiteTemplates]),
    Function(name="ParseSource", consumes=[SourceFile], produces=[Article, FSError]),
    Function(
        name="RenderArticle",
        consumes=[Article, SiteTemplates, Settings],
        produces=[HTML],
    ),
    Function(name="WriteHTML", consumes=[HTML], produces=[FSError]),
]

events = validate_and_trace([InitialCommand, SourceFile], my_flow)
generate_loose_graph(events)
