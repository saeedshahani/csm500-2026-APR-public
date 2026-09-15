use std::io::Write;
use std::process::{Command, Output, Stdio};
fn invoke(command: &str, input: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_cli"))
        .args([command, "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}
#[test]
fn compiled_lambda_commands_preserve_meaning_and_distinguish_input_resource_failures() {
    let compiled = invoke("lambda-compile", "((\\x. x) (\\y. y))");
    assert!(compiled.status.success());
    let readback = invoke(
        "lambda-readback",
        std::str::from_utf8(&compiled.stdout).unwrap(),
    );
    assert!(readback.status.success());
    assert_eq!(
        std::str::from_utf8(&readback.stdout).unwrap().trim(),
        "((λ.0) (λ.0))"
    );
    assert_eq!(invoke("lambda-compile", "(\\x. y)").status.code(), Some(2));
    assert_eq!(
        invoke("lambda-compile", &" ".repeat(200_001)).status.code(),
        Some(3)
    );
    assert_eq!(
        invoke("lambda-readback", "program {}").status.code(),
        Some(2)
    );
    let help = Command::new(env!("CARGO_BIN_EXE_cli"))
        .args(["lambda-compile", "--help"])
        .output()
        .unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8(help.stdout)
        .unwrap()
        .contains("Lambda adapters"));
}
