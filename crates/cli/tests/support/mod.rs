use std::io::{ErrorKind, Write};
use std::process::{Child, Output};

/// CLI argument validation can reject a request before consuming stdin.
/// A broken pipe is acceptable only for a normal, non-zero child exit; callers
/// still assert the exact expected status and report. Successful commands must
/// accept the complete input, and other I/O failures remain test failures.
pub fn finish_with_input(mut child: Child, input: &[u8]) -> Output {
    let written = child.stdin.take().expect("piped stdin").write_all(input);
    let output = child.wait_with_output().expect("collect CLI output");
    if let Err(error) = written {
        assert_eq!(error.kind(), ErrorKind::BrokenPipe, "stdin write: {error}");
        assert!(
            matches!(output.status.code(), Some(code) if code != 0),
            "child closed stdin without normal rejection: {output:?}"
        );
    }
    output
}
