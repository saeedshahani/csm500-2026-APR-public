mod support;
use std::process::{Command, Stdio};

fn child(args: &[&str]) -> std::process::Child {
    Command::new(env!("CARGO_BIN_EXE_cli"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

#[test]
fn early_argument_rejection_can_close_a_large_pending_stdin_write() {
    // Exceeds the supported hosts' pipe buffers; the invalid option is rejected
    // before any of these bytes are read, deterministically exercising closure.
    let input = vec![b' '; 8 * 1024 * 1024];
    for command in ["trace-provenance", "trace-cost", "check"] {
        let output =
            support::finish_with_input(child(&[command, "-", "--unknown-test-option"]), &input);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn successful_command_consumes_its_complete_input() {
    let output = support::finish_with_input(child(&["check", "-"]), b"program {}");
    assert!(output.status.success(), "{output:?}");
}

#[test]
#[should_panic(expected = "child closed stdin without normal rejection")]
fn broken_pipe_cannot_disguise_a_successful_early_exit() {
    support::finish_with_input(child(&["--help"]), &vec![b' '; 8 * 1024 * 1024]);
}
