import uuid
from typing import List, Dict, Any
from pydantic import BaseModel
from pyvis.network import Network


# --- 1. Models ---
class Type(BaseModel):
    name: str
    is_mutable: bool = True
    is_collection: bool = False
    origin_uid: int
    destination_uid: int | None = None

    def __hash__(self):
        return hash((self.name, self.is_mutable))

    def __eq__(self, other):
        return (
            isinstance(other, Type)
            and self.name == other.name
            and self.is_mutable == other.is_mutable
            and self.is_collection == other.is_collection
        )


class Data(Type):
    pass


class Error(Type):
    pass


class Function(BaseModel):
    name: str
    uid: int
    consumes: List[Type]
    produces: List[Type]


# --- 2. Definitions ---
InitialCommand = Data(name="InitialCommand")
PathToConfiguration = Data(name="PathToConfiguration")
SourceFile = Data(name="SourceFile", is_mutable=False, is_collection=True)
SourceFiles = Data(name="SourceFiles", is_mutable=False)
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
    produces=[SourceFiles, FSError],
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


# Pool now stores dictionaries instead of TokenInstance objects
def create_token(t, is_coll=False, is_in=False):
    return {
        "id": f"node_{uuid.uuid4().hex[:8]}",
        "data_type": t,
        "name": t.name,
        "is_mutable": t.is_mutable,
        "is_collection": is_coll,
        "is_input": is_in,
    }


class Node(BaseModel):
    name: str
    is_start: bool = False
    is_end: bool = False


# --- 3. Logic Engine ---
def process_flow(flow: List[Function]) -> List[Dict[str, Any]]:
    start_node = Node(name="Start", is_start=True)
    end_node = Node(name="End", is_end=True)
    nodes = [start_node]

    pool = flow[0].consumes

    for i, func in enumerate(flow, 1):
        for type_in in func.consumes:
            if type_in.is_mutable:
                pool.remove(type_in)

        for type_out in func.produces:
            if type_out.is_mutable or type_out not in pool:
                pool.append(type_out)
        print(f"{func.name}\n{[t.name for t in pool]}\n")

    nodes.append(end_node)
    return nodes


# --- 4. Visualizer ---
def generate_graph(flow: List[Function], filename="architecture.html"):
    net = Network(
        height="900px",
        width="100%",
        bgcolor="#121212",
        font_color="white",
        directed=True,
    )
    # added_tokens = set()

    for i, entry in enumerate(flow, 1):
        net.add_node(
            i,
            label=entry.name,
            shape="box",
            color="#2921FF"
            if "start" == entry.name.lower() or "end" == entry.name.lower()
            else "#FF5722",
            size=15,
            # borderWidth=2 if entry["is_iterative"] else 1,
        )

        # net.add_edge()

        # for t in entry["consumed"] + entry["produced"]:
        #     if t["id"] not in added_tokens:
        #         # Color logic based on Class types
        #         if isinstance(t["data_type"], Error):
        #             base_color = "#ff7575"
        #         elif not t["is_mutable"]:
        #             base_color = "#fccb6f"
        #         else:
        #             base_color = "#8de3a1"

        #         node_params = {
        #             "label": f"{t['name']}[]" if t["is_collection"] else t["name"],
        #             "shape": "dot",
        #             "color": base_color,
        #             "size": 12,
        #             "mass": 1,
        #         }
        #         if t["is_collection"]:
        #             node_params.update(
        #                 {
        #                     "size": 15,
        #                     "borderWidth": 3,
        #                     "color": {"border": "#ffffff", "background": base_color},
        #                 }
        #             )

        #         # --- START/END NODES POSITIONING (COMMENTED OUT) ---
        #         # if t["is_input"]: node_params.update({"x": -600, "fixed": True, "mass": 2})

        #         net.add_node(t["id"], **node_params)
        #         added_tokens.add(t["id"])

        # for t in entry["consumed"]:
        #     e_color = "#444444" if not t["is_mutable"] else "#8de3a1"
        #     net.add_edge(
        #         t["id"],
        #         f_id,
        #         color=e_color,
        #         width=2.5 if t["is_collection"] else 1,
        #         dashes=[8, 4] if t["is_collection"] else False,
        #     )

        # for t in entry["produced"]:
        #     net.add_edge(
        #         f_id,
        #         t["id"],
        #         color="#00ffcc",
        #         width=2.5 if t["is_collection"] else 1,
        #         dashes=[8, 4] if t["is_collection"] else False,
        #     )

    net.set_options(
        '{"physics": {"barnesHut": {"gravitationalConstant": -6000, "springLength": 80, "avoidOverlap": 1}}}'
    )
    net.show(filename, notebook=False)


# --- 5. Run ---
ir_data = process_flow(my_flow)
generate_graph(my_flow)
