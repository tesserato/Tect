use std::fs;
use std::path::Path;

pub fn assert_output(path_str: &str, actual: String) {
    let path = Path::new(path_str);

    // Normalize newlines in actual content to ensure consistent comparison
    let actual_normalized = actual.replace("\r\n", "\n");

    if std::env::var("UPDATE_EXPECTED").is_ok() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("Failed to create expected output directory");
        }
        // Write the normalized version? Or raw?
        // usually best to write consistent \n, git handles conversion.
        fs::write(path, &actual_normalized).expect("Unable to write expected output");
        println!("Updated expected output: {}", path_str);
        return;
    }

    let expected = fs::read_to_string(path).unwrap_or_else(|_| {
        panic!(
            "Expected output file not found at: {}\nRun with UPDATE_EXPECTED=1 to create it.",
            path_str
        )
    });

    let expected_normalized = expected.replace("\r\n", "\n");

    if expected_normalized != actual_normalized {
        // Print a diff-like message or just the mismatch
        // Using assert_eq! gives a decent diff in typical test runners
        assert_eq!(
            expected_normalized, actual_normalized,
            "Output mismatch for file: {}. Run UPDATE_EXPECTED=1 to update.",
            path_str
        );
    }
}
