Tect is a minimal (only 6 keywords), type-safe language for software architecture with rich tooling. Define variables, constants, errors and functions declaratively, catch cycles and starvation early through simulation, and generate interactive diagrams directly from your source.

![Demo](https://github.com/tesserato/Tect/blob/main/art/demo.gif?raw=true "Demo")

## Features

- **Architecture as Code**: Define systems using a minimal, declarative syntax with only **6 keywords** (`constant`, `variable`, `error`, `group`, `function`, `import`).
- **Flow Simulation**: The engine simulates token consumption and production to verify that every function has the required inputs and every error is handled.
- **Live Visualization**: Interactive force-directed graphs to explore complex systems.
- **Universal Export**: Generate artifacts for any use case:
  - **HTML**: Interactive web graph with physics controls.
  - **Mermaid/DOT**: For embedding in Markdown/Wikis.
  - **LaTeX (TikZ)**: For academic papers and publication-quality PDFs.
  - **JSON**: For programmatic analysis.

## Quick Start

> [!WARNING]
> Early-stage project.
> Tect is very new and still evolving, so you may encounter rough edges, breaking changes, or incomplete features.
> Feedback and contributions are highly welcome — feel free to open issues, submit pull requests, or share ideas to help shape the project.

### 1. Installation

#### VS Code Extension
For the best experience, install the **Tect** [extension for VS Code](https://marketplace.visualstudio.com/items?itemName=tesserato.tect). It provides:
- Syntax highlighting and snippets.
- Live architecture preview.
- Go-to-definition (supports files and symbols).
- Real-time error reporting.
  

#### Alternatively, install the CLI via crates.io [crates.io](https://crates.io/crates/Tect)
```bash
cargo install Tect
```

### 2. Define Architecture (`system.tect`)
```tect
# Define artifacts
constant Config
variable UserData
error DbError

# Define groups
group Database
group API

# Define contracts
Database function LoadUser Config
    > UserData
    | DbError

API function Serve UserData
    > Response
```

### 3. CLI Usage
```bash
# Verify logic (check for cycles, starvation, unused symbols)
tect check system.tect

# Format code
tect fmt system.tect

# Generate interactive HTML graph
tect build system.tect -o architecture.html

# Generate LaTeX/TikZ for PDF
tect build system.tect -o architecture.tex
```

### 4. Examples

Some examples are available in the [Examples](https://github.com/tesserato/Tect/tree/main/examples) folder.




