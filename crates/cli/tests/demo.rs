use std::process::Command;

#[test]
fn compiled_cli_runs_the_walking_skeleton() {
    for args in [vec![], vec!["demo"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_cli"))
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap(),"SelfLower before: 23 atoms, rank 1\nSelfLower after: 22 atoms, rank 0; consumed 3, produced 2, revised 1 rule(s)\n");
    }
}

#[test]
fn unknown_cli_command_fails_with_usage() {
    let output = Command::new(env!("CARGO_BIN_EXE_cli"))
        .arg("unknown")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("usage: cli [demo]"));
}
