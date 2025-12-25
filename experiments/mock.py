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
import networkx as nx

# --- 1. Enhanced Types to support "Instances" ---
class Token:
    """A specific instance of a Type in the pool at a specific time."""
    def __init__(self, data_type):
        self.id = str(uuid.uuid4())[:8]
        self.data_type = data_type
        self.name = data_type.name
        self.is_mutable = data_type.is_mutable

# --- 2. The Logic-Aware Validator ---
def validate_and_trace(initial_types, flow):
    # The pool now contains unique Token objects
    pool = [Token(t) for t in initial_types]
    
    # We record events for the graph
    trace_events = [] # List of (function_name, consumed_tokens, produced_tokens)

    for func in flow:
        consumed = []
        # Find the specific tokens in the pool
        for req_type in func.consumes:
            # We look for a token matching this type name
            token = next((t for t in pool if t.name == req_type.name), None)
            
            if not token:
                raise ValueError(f"Missing {req_type.name} for {func.name}")
            
            consumed.append(token)
            if token.is_mutable:
                pool.remove(token)

        # Produce new tokens
        produced = []
        for prod_type in func.produces:
            # Engine Fix: Don't duplicate immutable state instances
            existing = next((t for t in pool if t.name == prod_type.name), None)
            if not prod_type.is_mutable and existing:
                produced.append(existing)
            else:
                new_token = Token(prod_type)
                pool.append(new_token)
                produced.append(new_token)
        
        trace_events.append((func.name, consumed, produced))
    
    return trace_events

# --- 3. The Visualizer ---
def generate_logic_graph(trace_events, filename="logic_trace.html"):
    net = Network(height="800px", width="100%", bgcolor="#1a1a1a", font_color="white", directed=True)
    
    # Track which nodes we've already added
    added_tokens = set()

    for i, (func_name, consumed, produced) in enumerate(trace_events):
        # Create a unique node for this specific function execution
        func_node_id = f"{func_name}_{i}"
        net.add_node(func_node_id, label=func_name, shape="diamond", color="#6fb1fc", size=20)

        # Connect consumed tokens to this function
        for t in consumed:
            if t.id not in added_tokens:
                color = "#ff7575" if "Error" in t.name else "#8de3a1"
                if not t.is_mutable: color = "#fccb6f"
                
                net.add_node(t.id, label=t.name, shape="dot", color=color, size=10)
                added_tokens.add(t.id)
            
            net.add_edge(t.id, func_node_id, color="#555555")

        # Connect function to produced tokens
        for t in produced:
            if t.id not in added_tokens:
                color = "#ff7575" if "Error" in t.name else "#8de3a1"
                if not t.is_mutable: color = "#fccb6f"

                net.add_node(t.id, label=t.name, shape="dot", color=color, size=10)
                added_tokens.add(t.id)
            
            net.add_edge(func_node_id, t.id, color="#00ffcc")

    # Physics settings to make "related stuff" group together
    net.set_options("""
    var options = {
      "physics": {
        "forceAtlas2Based": {
          "gravitationalConstant": -50,
          "centralGravity": 0.01,
          "springLength": 100,
          "springStrength": 0.08
        },
        "solver": "forceAtlas2Based"
      }
    }
    """)
    net.show(filename, notebook=False)

# --- 4. Run ---
events = validate_and_trace([InitialCommand, SourceFile], my_flow)
generate_logic_graph(events)