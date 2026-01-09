use crate::lsp::format_tect_source;
use std::fs;

#[test]
fn test_format_dsbg() {
    let input_path = "../examples/dsbg.tect";
    let content = fs::read_to_string(input_path).expect("Failed to read dsbg.tect");

    let formatted = format_tect_source(&content).expect("Failed to format content");

    // Output to file for verification
    let output_path = "../examples/expected_outputs/formatted_dsbg.tect";
    fs::write(output_path, &formatted).expect("Failed to write formatted file");

    println!("Formatted content written to {}", output_path);
}
