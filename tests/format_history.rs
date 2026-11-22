//! Keep historical fixture integrity in the ordinary workspace test run.
#[test]
fn frozen_history_bytes_match_the_committed_baselines() {
    let result = std::process::Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scripts/check_format_fixtures.py"
        ))
        .output()
        .expect("python3 is required for repository checks");
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
