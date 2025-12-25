from pydantic import BaseModel
from typing import List


class Type(BaseModel):
    name: str
    is_mutable: bool = True  # True = Consumed (Var), False = Persistent (Const)

    def __hash__(self):
        return hash((self.name, self.is_mutable))

    def __eq__(self, other):
        return self.name == other.name and self.is_mutable == other.is_mutable


class Function(BaseModel):
    name: str
    consumes: List[Type]
    produces: List[Type]


# --- 1. Define Your Data Types ---
# Mutable (Resources that get transformed)
InitialCommand = Type(name="InitialCommand")
PathToConfiguration = Type(name="PathToConfiguration")
SourceFile = Type(name="SourceFile")
Article = Type(name="Article")
HTML = Type(name="HTML")

# Immutable (Configuration/Environment that stays available)
Settings = Type(name="Settings", is_mutable=False)
SiteTemplates = Type(name="SiteTemplates", is_mutable=False)

# Errors
FSError = Type(name="FileSystemError")

# --- 2. Define Your Functions ---
ProcessInitialCommand = Function(
    name="ProcessInitialCommand", consumes=[InitialCommand], produces=[Settings, PathToConfiguration]
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

# --- 3. The Validator Engine ---
def validate_architecture(initial_pool: List[Type], flow: List[Function]):
    pool = initial_pool.copy()

    print("-" * 60)
    print(f"STARTING POOL: {[t.name for t in pool]}")
    print("-" * 60)

    for i, func in enumerate(flow, 1):
        print(f"\nSTEP {i}: {func.name}")

        # 1. Check and Consume
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

        # 2. Produce
        pool.extend(func.produces)
        if func.produces:
            print(f"  [+] Produced: {[p.name for p in func.produces]}")

        # 3. Print Complete Pool State
        pool_names = [t.name for t in pool]
        print(f"  >> CURRENT POOL: {pool_names}")

    print("\n" + "-" * 60)
    # Final Validation
    errors = [t for t in pool if "Error" in t.name]
    if errors:
        print(f"⚠️  CRITICAL FAILURE: Unhandled errors: {[e.name for e in errors]}")
    else:
        print("✅ SUCCESS: Flow complete. No unhandled errors.")
    print("-" * 60)


# --- 4. Run Example ---
# We start with the command and a source file to process


my_initial_data = my_flow[0].consumes

validate_architecture(my_initial_data, my_flow)
