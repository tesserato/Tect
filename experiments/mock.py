from pydantic import BaseModel
from typing import List, Set


class Type(BaseModel):
    name: str
    is_mutable: bool = True  # If False, it's accessed (Const), not consumed (Var)

    def __hash__(self):
        return hash((self.name, self.is_mutable))

    def __eq__(self, other):
        return self.name == other.name and self.is_mutable == other.is_mutable


class Function(BaseModel):
    name: str
    consumes: List[Type]
    produces: List[Type]


# --- 1. Define Data and Error Types ---
# Mutable Data (Consumed)
InitialCommand = Type(name="InitialCommand")
SourceFile = Type(name="SourceFile")
Article = Type(name="Article")
HTML = Type(name="HTML")

# Immutable Data (Accessed/Persistent)
# Once loaded, Settings and Templates stay in the pool for everyone to read
Settings = Type(name="Settings", is_mutable=False)
SiteTemplates = Type(name="SiteTemplates", is_mutable=False)

# Errors
FSError = Type(name="FileSystemError")

# --- 2. Define Functions ---
ProcessInitialCommand = Function(
    name="ProcessInitialCommand", consumes=[InitialCommand], produces=[Settings]
)

LoadTemplates = Function(
    name="LoadTemplates",
    consumes=[Settings],  # Accesses Settings (Const)
    produces=[SiteTemplates],
)

ParseSource = Function(
    name="ParseSource",
    consumes=[SourceFile],
    produces=[Article, FSError],  # Might fail
)

RenderArticle = Function(
    name="RenderArticle",
    consumes=[
        Article,
        SiteTemplates,
        Settings,
    ],  # Consumes Article, Accesses Templates/Settings
    produces=[HTML],
)

WriteHTML = Function(
    name="WriteHTML",
    consumes=[HTML],
    produces=[FSError],  # Might fail
)

# A function to "Clean up" or handle the errors
ErrorHandler = Function(name="ErrorHandler", consumes=[FSError], produces=[])


# --- 3. The Validator Engine ---
def validate_architecture(initial_pool: List[Type], flow: List[Function]):
    pool = initial_pool.copy()

    print(f"🚀 Starting Flow with: {[t.name for t in pool]}")

    for func in flow:
        print(f"\nStep: {func.name}")
        for req in func.consumes:
            if req not in pool:
                raise ValueError(
                    f"❌ ERROR: '{func.name}' needs '{req.name}', but it's not in the pool."
                )

            if req.is_mutable:
                pool.remove(req)
                print(f"  [-] Consumed: {req.name}")
            else:
                print(f"  [∞] Accessed: {req.name} (remains in pool)")

        pool.extend(func.produces)
        print(f"  [+] Produced: {[p.name for p in func.produces]}")

    # Final "Clean Pool" check
    errors = [t for t in pool if "Error" in t.name]
    if errors:
        print(
            f"\n⚠️  CRITICAL FAILURE: Flow ended with unhandled errors: {[e.name for e in errors]}"
        )
    else:
        print("\n✅ SUCCESS: Flow completed. All data consumed and errors handled.")


# --- 4. Testing the Architecture ---

# Scenario: A valid flow
validate_architecture(
    initial_pool=[InitialCommand, SourceFile],
    flow=[
        ProcessInitialCommand,
        LoadTemplates,
        ParseSource,
        RenderArticle,
        WriteHTML,
        ErrorHandler,  # If you comment this out, it will flag a Critical Failure
        ErrorHandler,  # Handles the second potential FSError
    ],
)
