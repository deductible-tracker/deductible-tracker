use std::process::Command;

#[test]
fn javascript_jest_suite_passes() {
    let output = Command::new("npm")
        .args(["run", "test:js"])
        .output()
        .expect("failed to execute npm; ensure Node.js and npm are installed");

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!(
            "Jest tests failed.\nstatus: {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            stdout,
            stderr
        );
    }
}
