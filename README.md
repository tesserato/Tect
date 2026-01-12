# Tect - Architectural Specification Language & Tooling

Tect is a lightweight (less than 10 keywords), type-safe language for defining software architectures as code. It compiles architectural definitions into logical graphs, simulates data flow to detect starvation or cycles, and generates high-quality diagrams for documentation and analysis.

## Features

- **Architecture as Code**: Define constants, variables, errors, and groups using a clean, declarative syntax.
- **Flow Simulation**: The engine simulates token consumption and production to verify that every function has the required inputs and every error is handled.
- **Live Visualization**: Interactive force-directed graphs to explore complex systems.
- **Universal Export**: Generate artifacts for any use case:
  - **HTML**: Interactive web graph with physics controls.
  - **Mermaid/DOT**: For embedding in Markdown/Wikis.
  - **LaTeX (TikZ)**: For academic papers and publication-quality PDFs.
  - **JSON**: For programmatic analysis.

## Quick Start

### 1. Installation

#### VS Code Extension
For the best experience, install the **Tect** [extension for VS Code](https://marketplace.visualstudio.com/items?itemName=tesserato.tect). It provides:
- Syntax highlighting and snippets.
- Live architecture preview.
- Go-to-definition (supports files and symbols).
- Real-time error reporting.
  

#### Alternatively, install CLI only via [crates.io](https://crates.io/crates/Tect)
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





