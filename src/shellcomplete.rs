use crate::clapargs::NeocmakeCli;

use clap::{Command, CommandFactory};
use clap_complete::{generate, Generator, Shell};

fn print_completions<G: Generator>(gen: G, cmd: &mut Command) {
    generate(gen, cmd, cmd.get_name().to_string(), &mut std::io::stdout());
}

pub fn generate_shell_completions(shell: Shell) {
    let mut cmd = NeocmakeCli::command();
    eprintln!("Generating completion file for {shell:?}...");
    print_completions(shell, &mut cmd);
}
