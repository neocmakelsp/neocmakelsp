use std::process::Command;

use assert_cmd::cargo::CommandCargoExt;
use regex::{Captures, Regex};

fn compare_shell_completions(shell: &str, completion_script: &str) {
    let mut command = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();
    command.env("COMPLETE", shell);

    let binary = command.get_program().to_str().unwrap().to_string();

    let output = command.output().unwrap();
    assert!(output.status.success(), "Failed to call neocmakelsp");

    let output = String::from_utf8_lossy(&output.stdout).to_string();

    // The completion scripts in the source tree only contain the `neocmakelsp` binary name,
    // however in this test the generated binary name is the absolute path to the binary, which
    // would not be portable and not ready to be shipped.
    // So we just replace the binary name with the absolute path here.
    let regex = Regex::new(r#"("?)(neocmakelsp)("?) --"#).unwrap();
    let completion_script = regex.replace(completion_script, |caps: &Captures<'_>| {
        format!("{}{binary}{} --", &caps[1], &caps[3])
    });

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
