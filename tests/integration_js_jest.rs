use std::process::{Command, Stdio};

#[test]
fn javascript_jest_suite_passes() {
    let status = Command::new("npm")
        .args(["run", "test:js"])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .expect("failed to execute npm; ensure Node.js and npm are installed");

    assert!(
        status.success(),
        "Jest tests failed with status: {:?}",
        status.code()
    );
}
