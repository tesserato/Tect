# tect
A meta-programming language for reasoning about code architecture.



- [ ] Hover description in keywords (error, data, etc)
- [ ] functions are always pure, data is always immutable
- [ ] format this document
- [ ] show errors, etc: make sure when parsing is broken


# kinds

data
error recuperable errors : warning if not dealt with
group
function
flow?
ok? 
panic? Irrecuperable error, aborts program

# keywords
loop?
match?

# types

data Table
data pathString
data BooleanShouldSave

error FileNotFound
error 
# functions

receive errors and or data and can output multiple combinations of errors and data

function loadTable(pathString) > Table | FileNotFound
function saveTable(Table, pathString) > ok | panic


The program maintains a pool of types (multiset).

Each function:

consumes some types from the pool (its inputs)

adds some types to the pool (its outcomes)

Execution proceeds step by step.

At the end of the flow:

the pool must be empty

except for errors

Any unconsumed error is fatal.



# Example login
what about this:

data Credentials
data Session
data UserProfile

error InvalidPassword
error DatabaseOffline
error UserNotFound

function Authenticate(Credentials)
  > Session
  | InvalidPassword
  | DatabaseOffline

function FetchProfile(Session)
  > UserProfile
  | UserNotFound
  | DatabaseOffline


Authenticate # Credentials > Session | InvalidPassword | DatabaseOffline
FetchProfile {
    InvalidPassword { stop }
    UserNotFound     { stop }
    DatabaseOffline  { retry }
}

# Example DSBG

mut data InitialCommand 
mut data PathToConfig 
data SourceFile 
mut data Article 
mut data Html 
data Settings 
data Templates 
data Success

error FSError
error InitialCommandMalformedError
error FileNotFoundError
error ConfigurationMalformedError
error FileSystemError
error MetadataError
error TemplateError

function ScanFS(Settings)
    > [SourceFile]
    | [FSError]


function ProcessInitialCommand(InitialCommand)
    > Configuration
    | PathToConfiguration
    | InitialCommandMalformedError

function ReadConfiguration(PathToConfiguration)
    > Configuration
    | FileNotFoundError
    | ConfigurationMalformedError

function PrepareOutput(Settings)
    > Settings
    | FileSystemError

function DeployStaticAssets(Settings)
    > Settings
    | FileSystemError

function ExtractMetadata(SourceFile)
    > Article
    | MetadataError

function ResolveResources(Article)
    > Article
    | FileSystemError

function RenderPage(Article)
    > String
    | TemplateError

function FinalizeSite(Settings)
    > String
    | TemplateError


ProcessInitialCommand
ReadConfiguration


PrepareOutput
DeployStaticAssets


ExtractMetadata
ResolveResources
RenderPage


FinalizeSite

# Tect Language Specification (v0.0.0)

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

# TODO
make syntax and semantic highlight compatible - offload as much as possible to syntax

integration tests: run all examples in samples folder (rename to examples), through all possible means, and compare outputs with pre curated default files

remove hardcoded html

add icon (vscode files too)

improve CLI help message

is it possible to track usage of variables and constants globally?

right click export to formats from graph in vs code?

button to stop graph from rotatin

better colors for groups (more visible, and different between themselves)

expand all collapse all toggle

optional custom config for vs code graph

click on import path to jump to file
