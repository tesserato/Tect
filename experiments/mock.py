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
