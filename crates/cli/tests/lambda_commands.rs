mod support;
use std::process::{Command, Output, Stdio};
fn invoke(command: &str, input: &str) -> Output {
    invoke_args(&[command, "-"], input)
}
fn invoke_args(args: &[&str], input: &str) -> Output {
    let child = Command::new(env!("CARGO_BIN_EXE_cli"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    support::finish_with_input(child, input.as_bytes())
}

#[test]
fn open_profile_commands_preserve_symbols_and_reject_ambiguous_options() {
    let compiled = invoke_args(
        &[
            "lambda-compile",
            "-",
            "--profile",
            "open-cbn-v1",
            "--free",
            "x",
        ],
        "((\\z. (\\x. z)) x)",
    );
    assert!(compiled.status.success());
    let source = std::str::from_utf8(&compiled.stdout).unwrap();
    let read = invoke("lambda-readback", source);
    assert!(read.status.success());
    assert_eq!(
        std::str::from_utf8(&read.stdout).unwrap().trim(),
        "((λ.(λ.1)) x)"
    );
    for options in [
        vec!["--profile"],
        vec!["--profile", "wrong"],
        vec!["--free", "x"],
        vec!["--profile", "open-cbn-v1", "--free", "x,x"],
        vec!["--profile", "open-cbn-v1", "--profile", "open-cbn-v1"],
    ] {
        let mut args = vec!["lambda-compile", "-"];
        args.extend(options);
        assert_eq!(invoke_args(&args, "x").status.code(), Some(2));
    }
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
