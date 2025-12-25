import uuid
from typing import List, Tuple
from pydantic import BaseModel
from pyvis.network import Network


# --- 1. Unified Models ---
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
    """Represents a unique instance of a Type for tracing."""

    def __init__(self, data_type: Type):
        self.id = f"node_{uuid.uuid4().hex[:8]}"
        self.name = data_type.name
        self.is_mutable = data_type.is_mutable


# --- 2. The Consolidated Engine ---
def process_flow(initial_types: List[Type], flow: List[Function]) -> List[Tuple]:
    """Validates logic, prints status, and returns trace events for the graph."""
    pool = [TokenInstance(t) for t in initial_types]
    trace_events = []

    print(f"{'='*20} STARTING FLOW {'='*20}")
    print(f"Initial Pool: {[t.name for t in pool]}\n")

    for i, func in enumerate(flow, 1):
        consumed_tokens = []
        for req in func.consumes:
            # Find matching token in pool
            token = next((t for t in pool if t.name == req.name), None)
            if not token:
                raise ValueError(
                    f"❌ STEP {i}: '{func.name}' missing requirement '{req.name}'"
                )

            consumed_tokens.append(token)
            if token.is_mutable:
                pool.remove(token)

        produced_tokens = []
        for prod in func.produces:
            existing = next((t for t in pool if t.name == prod.name), None)
            if not prod.is_mutable and existing:
                produced_tokens.append(existing)
            else:
                new_token = TokenInstance(prod)
                pool.append(new_token)
                produced_tokens.append(new_token)

        trace_events.append((func.name, consumed_tokens, produced_tokens))

        # Console Output
        print(f"STEP {i}: {func.name}")
        print(
            f"  [-] Consumed: {[t.name for t in consumed_tokens if t.is_mutable] or 'None'}"
        )
        print(f"  [+] Produced: {[t.name for t in produced_tokens]}")
        print(f"  >> POOL: {[t.name for t in pool]}\n")

    # Final error check
    unhandled = [t.name for t in pool if "Error" in t.name]
    if unhandled:
        print(f"⚠️  CRITICAL FAILURE: Unhandled errors: {unhandled}")
    else:
        print("✅ SUCCESS: Flow completed safely.")

    return trace_events


# --- 3. The Visualizer ---
def generate_graph(trace_events: List[Tuple], filename="architecture.html"):
    net = Network(
        height="900px",
        width="100%",
        bgcolor="#121212",
        font_color="white",
        directed=True,
        layout=True,
    )
    added_tokens = set()

    for i, (f_name, consumed, produced) in enumerate(trace_events):
        f_id = f"func_{i}"
        net.add_node(
            f_id, label=f_name, shape="diamond", color="#6fb1fc", size=25, mass=2
        )

        for t in consumed + produced:
            if t.id not in added_tokens:
                color = (
                    "#ff7575"
                    if "Error" in t.name
                    else ("#fccb6f" if not t.is_mutable else "#8de3a1")
                )
                net.add_node(t.id, label=t.name, shape="dot", color=color, size=15)
                added_tokens.add(t.id)

        for t in consumed:
            net.add_edge(t.id, f_id, color="#444444")
        for t in produced:
            net.add_edge(f_id, t.id, color="#00ffcc")

    net.set_options(
        '{"physics": {"barnesHut": {"gravitationalConstant": -10000, "springLength": 150}}}'
    )
    net.show(filename, notebook=False)


# --- 4. Data Setup & Execution ---
# Define Types
T = {
    "cmd": Type(name="InitialCommand"),
    "path": Type(name="PathToConfiguration"),
    "src": Type(name="SourceFile"),
    "art": Type(name="Article"),
    "html": Type(name="HTML"),
    "set": Type(name="Settings", is_mutable=False),
    "tmp": Type(name="SiteTemplates", is_mutable=False),
    "err": Type(name="FileSystemError"),
}

# Define Flow
my_flow = [
    Function(
        name="ProcessInitialCommand",
        consumes=[T["cmd"]],
        produces=[T["set"], T["path"]],
    ),
    Function(name="LoadConfiguration", consumes=[T["path"]], produces=[T["set"]]),
    Function(name="LoadTemplates", consumes=[T["set"]], produces=[T["tmp"]]),
    Function(
        name="FindSourceFiles", consumes=[T["set"]], produces=[T["src"], T["err"]]
    ),
    Function(name="ParseSource", consumes=[T["src"]], produces=[T["art"], T["err"]]),
    Function(
        name="RenderArticle",
        consumes=[T["art"], T["tmp"], T["set"]],
        produces=[T["html"]],
    ),
    Function(name="WriteHTML", consumes=[T["html"]], produces=[T["err"]]),
]

# Run
trace = process_flow([T["cmd"]], my_flow)
generate_graph(trace)
