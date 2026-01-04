This updated specification for the **Tect** language is derived from the logic in `test_engine.rs` and the syntax patterns found in `dsbg.tect`. It formalizes the shift from the `data` keyword to explicit `constant`/`variable` declarations and clarifies the architectural flow mechanics.

### **Features Added**
*   **Explicit Mutability**: Replaced the `data` keyword with `constant` (immutable/persistent) and `variable` (mutable/linear).
*   **Cardinality Formalization**: Defined how `[Type]` (Collection) vs `Type` (Unitary) affects flow expansion.
*   **Branching Logic**: Clarified how pipe-separated (`|`) outputs create isolated architectural pools.
*   **Terminal States**: Defined the `FinalNode` (Success) and `FatalErrors` (Unhandled) logic.

### **Features Removed**
*   **`data` keyword**: This keyword is no longer part of the language definition.

---

# Tect Language Specification (v1.1)

Tect is a domain-specific language for modeling architectural data flows. It focuses on how information is consumed, transformed, and propagated through a system, treating architecture as a **Directed Acyclic Graph of Token Pools**.

## 1. Naming & Lexical Rules
*   **Types (Definitions)**: Must start with an **Uppercase** letter (`Settings`, `FileSystemError`).
*   **Logic (Functions/Groups)**: Must start with an **Uppercase** letter in definitions to match the grammar, though the underlying engine treats them as logical identifiers.
*   **Comments**: Lines starting with `#` are comments.
*   **Doc-Comments**: Comments immediately preceding a definition or statement are captured as metadata for visualization tooltips.

## 2. Type Definitions
Definitions establish the "artifacts" that exist within the system's scope.

### 2.1 Artifact Mutability
*   **`constant`**: Immutable data. Once a constant is in the pool, it can be read by any number of functions. It is never "consumed" or removed.
*   **`variable`**: Mutable data. When a function takes a `variable` as input, that token is **moved** out of the pool. If the data needs to persist, the function must explicitly output it again.
*   **`error`**: Architectural failure states. These behave like **variables**; they must be consumed by a function (error handler) or they will result in a fatal state.

```tect
constant Settings          # Persists globally
variable InitialCommand    # Consumed upon use
error FileSystemError      # Must be handled or becomes Fatal
```

### 2.2 Groups
Groups define logical boundaries or tiers. Functions defined within a group are visually clustered together.
```tect
group Ingestion
group Environment
```

## 3. Function Contracts
Functions define the transformation logic and branching possibilities of the architecture.

### 3.1 Syntax
`[GroupName] function Name(Input1, Input2, ...) > OutputGroupA | OutputGroupB`

### 3.2 Cardinality & Expansion
*   **Unitary (`Type`)**: The function requires a single instance of the token.
*   **Collection (`[Type]`)**: The function requires the entire set of available tokens of that type.
*   **Expansion Mechanic**: If a function requests a **Unitary** token but only a **Collection** is available in the pool, the Tect engine triggers an **Expansion**. This represents a loop or parallel process where the function is executed once for every item in the collection.

### 3.3 Output Branching
Outputs are grouped by line or separated by pipes (`|`).
*   The `>` indicates the primary production.
*   Each `|` represents an **Alternative Branch**. The engine creates a separate "Pool" for each branch, allowing the architecture to represent success and failure paths simultaneously.

```tect
# Consumes a collection, produces a collection
function ScanFS(Settings) 
    > [SourceFile] 
    | [FileSystemError]

# Consumes a single item (triggers expansion if SourceFile is a collection)
function ParseMarkdown(SourceFile)
    > Article
    | FileSystemError
```

## 4. Flows
A Flow is a sequence of function identifiers. The engine executes them in order, attempting to satisfy their input requirements from the current **Token Pool**.

### 4.1 The Execution Cycle
1.  **Seed**: The flow starts at an `InitialNode` which populates the pool with the inputs required by the first function.
2.  **Consumption**: For every function in the flow:
    *   The engine looks for matching types in the pool.
    *   `variable` and `error` tokens are removed. `constant` tokens are referenced.
3.  **Production**: The function adds its outputs to the pool. If multiple output groups exist (pipes), the flow branches into multiple parallel pool states.

### 4.2 Terminal Routing
At the end of the flow, the engine performs a cleanup:
*   **Terminal Success**: Any remaining `variable` or `constant` tokens are routed to the `FinalNode`.
*   **Terminal Failure**: Any unconsumed `error` tokens are routed to the `FatalErrors` node.

## 5. Full Architectural Example
```tect
# Definitions
constant Settings
variable InitialCommand
error FileSystemError

# Contracts
group Environment function ProcessCLI(InitialCommand)
    > Settings
    | FileSystemError

group Ingestion function ScanFS(Settings)
    > [SourceFile]
    | [FileSystemError]

# Flow
ProcessCLI
ScanFS
```

---

### Behavior Matrix

| Declaration | Persistence | Consumption Rule | Terminal Target |
| :--- | :--- | :--- | :--- |
| `constant` | **Permanent** | Reference-only (Never removed) | `FinalNode` |
| `variable` | **Linear** | Move-semantics (Removed on use) | `FinalNode` |
| `error` | **Linear** | Move-semantics (Removed on use) | `FatalErrors` |
| `Type` | **Unitary** | Requires 1 item; triggers expansion if Collection found | N/A |
| `[Type]` | **Collection** | Requires all items of type | N/A |