use std::io::Write;

use clap::{Command, CommandFactory};
use clap_complete::{generate, Generator, Shell};

use crate::clapargs::NeocmakeCli;

fn print_completions<G: Generator>(gen: G, cmd: &mut Command, write: &mut dyn Write) {
    generate(gen, cmd, cmd.get_name().to_string(), write)
}

pub fn generate_shell_completion(shell: Shell) {
    let mut cmd = NeocmakeCli::command();
    eprintln!("Generating completion file for {shell:?}...");
    print_completions(shell, &mut cmd, &mut std::io::stdout());
}

// Seems on windows it is not the same
#[cfg(unix)]
#[cfg(test)]
mod target_test {
    use std::io::Cursor;

    use clap::CommandFactory;
    use clap_complete::Shell;

    use super::{print_completions, NeocmakeCli};
    #[test]
    fn tst_fish_is_same() {
        let mut cmd = NeocmakeCli::command();
        let fish = include_bytes!("../completions/fish/neocmakelsp.fish");
        let mut buffer = Cursor::new(Vec::new());
        print_completions(Shell::Fish, &mut cmd, &mut buffer);

        assert_eq!(buffer.get_ref(), fish);
    }
    #[test]
    fn tst_zsh_is_same() {
        let mut cmd = NeocmakeCli::command();
        let zsh = include_bytes!("../completions/zsh/_neocmakelsp");
        let mut buffer = Cursor::new(Vec::new());
        print_completions(Shell::Zsh, &mut cmd, &mut buffer);

        assert_eq!(buffer.get_ref(), zsh);
    }
    #[test]
    fn tst_bash_is_same() {
        let mut cmd = NeocmakeCli::command();
        let bash = include_bytes!("../completions/bash/neocmakelsp");
        let mut buffer = Cursor::new(Vec::new());
        print_completions(Shell::Bash, &mut cmd, &mut buffer);

        assert_eq!(buffer.get_ref(), bash);
    }
}
