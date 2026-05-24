//! CLI-level validation tests. These exercise the argument handling that runs
//! before any `.fig` is read, so they need no fixture file.

use std::process::Command;

fn fig2json() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fig2json"))
}

#[test]
fn rejects_invalid_node_id_before_reading_input() {
    let output = fig2json()
        .args(["--node", "not-an-id", "/no/such/file.fig"])
        .output()
        .expect("failed to run fig2json");

    assert!(!output.status.success(), "expected a non-zero exit");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("sessionID:localID"),
        "error should explain the expected id form; got: {stderr}"
    );
}

#[test]
fn rejects_raw_combined_with_node() {
    let output = fig2json()
        .args(["--raw", "--node", "1:2", "/no/such/file.fig"])
        .output()
        .expect("failed to run fig2json");

    assert!(!output.status.success(), "expected a non-zero exit");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--raw cannot be combined with --node"),
        "error should explain the --raw/--node conflict; got: {stderr}"
    );
}
