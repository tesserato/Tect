# Tect for VS Code

The official Visual Studio Code extension for the **Tect** architectural specification language.

## Features

### 1. Live Architecture Preview
Visualize your architecture in real-time as you type.
- **Command**: `Tect: Open Architecture Preview`
- **Interactive**: Drag nodes, pan, and zoom.
- **Toolbar**: Pause physics, fit graph, and expand/collapse groups.
- **Export**: Right-click the graph to save as **PNG**, **HTML**, **Mermaid**, **DOT**, or **LaTeX**.

### 2. Intelligent Editing
- **Syntax Highlighting**: Distinct colors for constants, variables, errors, and flow steps.
- **Go to Definition**: Ctrl+Click on symbols or `import` paths to jump to their definitions.
- **Diagnostics**: Real-time reporting of syntax errors, import cycles, and logic starvation.
- **Formatting**: Auto-format your code (`Shift+Alt+F`) to standard Tect style.

## Requirements

This extension requires the `tect` language server binary.
1. If you have Rust installed, the extension will attempt to use a locally compiled binary.
2. Ensure the `tect` binary is in your system PATH or the extension bundle.

## Extension Settings

You can customize the visual graph physics and layout via your `settings.json`:

```json
{
  "tect.visConfig": {
    "physics": {
      "barnesHut": {
        "gravitationalConstant": -2000,
        "springLength": 150
      }
    },
    "layout": {
      "improvedLayout": true
    }
  }
}
```

## Release Notes

### 0.0.4
- Added "Go to Definition" for import paths.
- Added Right-Click context menu for exporting graph to multiple formats.
- Improved graph color palette for better visibility.
- Added toolbar controls for physics and clustering.

### 0.0.1
- Initial release with syntax highlighting and basic LSP support.