use super::common::assert_output;
use crate::engine::Flow;
use crate::vis_js;
use std::fs;
use std::path::PathBuf;

#[test]
fn generate_blog_architecture_json() -> std::io::Result<()> {
    // 1. Read and Analyze source file
    let input_path = "../examples/dsbg.tect";
    let content = fs::read_to_string(input_path).expect("Failed to read dsbg.tect");
    let path = PathBuf::from(input_path);

    let mut workspace = crate::analyzer::Workspace::new();
    workspace.analyze(path, Some(content.clone()));
    let structure = &workspace.structure;

    // 2. Simulate Flow
    let mut flow = Flow::new(true);
    let graph = flow.simulate(structure);

    // 3. Serialize artifacts
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");

    // A) Architecture JSON
    let mut arch_buf = Vec::new();
    let mut arch_ser = serde_json::Serializer::with_formatter(&mut arch_buf, formatter.clone());
    serde::Serialize::serialize(&graph, &mut arch_ser).unwrap();
    let arch_json = String::from_utf8(arch_buf).expect("Generated JSON was not valid UTF-8");

    // B) Functions JSON
    let mut functions: Vec<_> = structure.catalog.values().collect();
    functions.sort_by_key(|f| f.uid);

    let mut func_buf = Vec::new();
    let mut func_ser = serde_json::Serializer::with_formatter(&mut func_buf, formatter);
    serde::Serialize::serialize(&functions, &mut func_ser).unwrap();
    let func_json = String::from_utf8(func_buf).expect("Generated JSON was not valid UTF-8");

    // C) HTML Output
    let html_content = vis_js::generate_interactive_html(&graph);

    // 4. Save artifacts
    let output_dir = "../examples/test_outputs";
    fs::create_dir_all(output_dir)?;

    fs::write(format!("{}/architecture.json", output_dir), &arch_json)?;
    fs::write(format!("{}/functions.json", output_dir), &func_json)?;
    fs::write(format!("{}/architecture.html", output_dir), &html_content)?;

    // 5. Compare against expected_outputs
    assert_output("../examples/expected_outputs/architecture.json", arch_json);
    assert_output("../examples/expected_outputs/functions.json", func_json);
    // assert_output(
    //     "../examples/expected_outputs/architecture.html",
    //     html_content,
    // );

    Ok(())
}
