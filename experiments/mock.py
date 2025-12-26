import itertools
from typing import List, Optional
from pydantic import BaseModel, Field
from pyvis.network import Network

# Counters for automatic ID generation
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


class TypeInstance(Type):
    origin_function_uid: int | None = None
    destination_function_uid: int | None = None

    @classmethod
    def from_type(
        cls, t: Type, origin: Optional[int] = None, destination: Optional[int] = None
    ):
        return cls(
            **t.model_dump(),
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


# --- 2. Definitions ---
InitialCommand = Data(name="InitialCommand")
PathToConfiguration = Data(name="PathToConfiguration")
SourceFile = Data(name="SourceFile", is_mutable=False, is_collection=True)
# SourceFiles = Data(name="SourceFiles", is_mutable=False)
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
def process_flow(flow: List[Function]) -> tuple[List[Function], List[TypeInstance]]:
    start_node = Function(name="Start", is_start=True)
    end_node = Function(name="End", is_end=True)
    nodes = [start_node]
    edges = []

    pool = [TypeInstance.from_type(n, origin=start_node.uid) for n in flow[0].consumes]

    print(pool)

    for func in flow:
        nodes.append(func)
        for t in func.consumes:
            type_instance = TypeInstance.from_type(t, destination=func.uid)
            if type_instance.is_mutable:
                pool.remove(type_instance)
            edges.append(type_instance)

        for type_out in func.produces:
            type_instance = TypeInstance.from_type(type_out, origin=func.uid)
            if type_instance.is_mutable or type_instance not in pool:
                pool.append(type_instance)
        print(func.name)
        # }\n{[t.name for t in pool]}\n"
        for p in pool:
            print("  ", p)
        print()

    for t in pool:
        t.destination_function_uid = end_node.uid
        edges.append(t)
    nodes.append(end_node)
    for edge in edges:
        print(edge)
    exit()
    return nodes, edges


# --- 4. Visualizer ---
def generate_graph(
    nodes: List[Function], edges: List[TypeInstance], filename="architecture.html"
):
    net = Network(
        height="900px",
        width="100%",
        bgcolor="#121212",
        font_color="white",
        directed=True,
    )
    # added_tokens = set()

    for node in nodes:
        net.add_node(
            node.uid,
            label=node.name,
            shape="box",
            color="#2921FF" if node.is_start or node.is_end else "#FF5722",
            size=15,
            # borderWidth=2 if entry["is_iterative"] else 1,
        )

    for e in edges:
        net.add_edge(
            e.origin_function_uid,
            e.destination_function_uid,
            label=e.name + ("[]" if e.is_collection else ""),
            color="#00ffcc" if e.is_mutable else "#444444",
            width=2.5 if e.is_collection else 1,
            dashes=[8, 4] if e.is_collection else False,
        )

    net.show(filename, notebook=False)


# --- 5. Run ---
nodes, edges = process_flow(my_flow)
generate_graph(nodes, edges)
