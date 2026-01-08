#![cfg(unix)]

use assert_cmd::cargo::cargo_bin_cmd;

fn compare_shell_completions(shell: &str, completion_script: &str) {
    let mut command = cargo_bin_cmd!();
    command.env("COMPLETE", shell);

    let output = command.output().unwrap();
    assert!(output.status.success(), "Failed to call neocmakelsp");

    let output = String::from_utf8_lossy(&output.stdout).to_string();

    assert_eq!(output, completion_script);
}

#[test]
fn verify_bash_completions() {
    compare_shell_completions("bash", include_str!("../completions/bash/neocmakelsp"));
}

#[test]
fn verify_zsh_completions() {
    compare_shell_completions("zsh", include_str!("../completions/zsh/_neocmakelsp"));
}
