use clap::{arg,  Command, CommandFactory, Parser};
use clap_complete::{generate, Generator, Shell};
use std::io;

mod clapargs;

use clapargs::NeocmakeCli;

#[derive(Parser, Debug, PartialEq)]
#[command(name = "completion-derive")]
struct Opt {
    // If provided, outputs the completion file for given shell
    #[arg(long = "generate", value_enum)]
    generator: Option<Shell>,
}

fn print_completions<G: Generator>(gen: G, cmd: &mut Command) {
    generate(gen, cmd, cmd.get_name().to_string(), &mut io::stdout());
}

fn main() {
    let opt = Opt::parse();

    if let Some(generator) = opt.generator {
        let mut cmd = NeocmakeCli::command();
        eprintln!("Generating completion file for {generator:?}...");
        print_completions(generator, &mut cmd);
    } else {
        println!("{opt:#?}");
    }
}
