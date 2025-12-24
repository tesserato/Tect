use std::fmt::Write;

/// Wraps a DOT graph into a self-contained interactive HTML page.
///
/// Uses Viz.js (ASM.js Graphviz) instead of WASM so the file:
/// - works from file://
/// - works offline
/// - does not rely on Web Workers
pub fn wrap_dot(dot: &str) -> String {
    let mut out = String::new();

    let dot_json = serde_json::to_string(dot).expect("DOT serialization failed");

    writeln!(
        out,
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8"/>
<title>Tect Architecture Graph</title>

<style>
    body {{
        margin: 0;
        background: #0f1115;
        color: #e6e6e6;
        font-family: Inter, system-ui, sans-serif;
        overflow: hidden;
    }}
    #graph {{
        width: 100vw;
        height: 100vh;
    }}
    svg {{
        width: 100%;
        height: 100%;
    }}
</style>
</head>

<body>
<div id="graph"></div>

<!-- D3 for zoom/pan -->
<script src="https://unpkg.com/d3@7"></script>

<!-- Viz.js (ASM.js Graphviz, no WASM, no workers) -->
<script src="https://unpkg.com/viz.js@2.1.2/viz.js"></script>
<script src="https://unpkg.com/viz.js@2.1.2/full.render.js"></script>

<script>
const dot = {dot_json};

const viz = new Viz();

viz.renderSVGElement(dot)
  .then(svg => {{
    const container = document.getElementById("graph");
    container.appendChild(svg);

    const zoom = d3.zoom()
      .scaleExtent([0.1, 4])
      .on("zoom", (event) => {{
        d3.select(svg).select("g").attr("transform", event.transform);
      }});

    d3.select(svg)
      .call(zoom)
      .call(zoom.transform, d3.zoomIdentity.scale(0.9));
  }})
  .catch(err => {{
    console.error("Graphviz render failed:", err);
  }});
</script>

</body>
</html>"#
    )
    .unwrap();

    out
}
