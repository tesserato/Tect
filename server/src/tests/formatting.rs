use crate::formatter::format_tect_source;

use super::common::assert_output;
use std::fs::{self, File};
use std::io::Write;

#[test]
fn test_format_dsbg() {
    let input_path = "../examples/dsbg.tect";
    let content = fs::read_to_string(input_path).expect("Failed to read dsbg.tect");

    let formatted = format_tect_source(&content).expect("Failed to format content");
    let mut output = File::create("../examples/test_outputs/formatted_dsbg.tect")
        .expect("Failed to create ../examples/test_outputs/formatted_dsbg.tect");
    write!(output, "{}", formatted)
        .expect("Failed to write ../examples/test_outputs/formatted_dsbg.tect");

    assert_output(
        "../examples/expected_outputs/formatted_dsbg.tect",
        formatted,
    );
}
