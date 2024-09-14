use std::io::Write;

use crate::clapargs::NeocmakeCli;

use clap::{Command, CommandFactory};
use clap_complete::{generate, Generator, Shell};

fn print_completions<G: Generator>(gen: G, cmd: &mut Command, write: &mut dyn Write) {
    generate(gen, cmd, cmd.get_name().to_string(), write)
}

pub fn generate_shell_completions(shell: Shell) {
    let mut cmd = NeocmakeCli::command();
    eprintln!("Generating completion file for {shell:?}...");
    print_completions(shell, &mut cmd, &mut std::io::stdout());
}
#[cfg(test)]
mod target_test {
    use super::{print_completions, NeocmakeCli};
    use clap::CommandFactory;
    use clap_complete::Shell;
    use std::io::Cursor;
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
