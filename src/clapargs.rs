use std::path::PathBuf;

use clap::{arg, Parser};
use clap_complete::Shell;

const LSP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn get_styles() -> clap::builder::Styles {
    clap::builder::Styles::styled()
        .usage(
            anstyle::Style::new()
                .bold()
                .underline()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Yellow))),
        )
        .header(
            anstyle::Style::new()
                .bold()
                .underline()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Yellow))),
        )
        .literal(
            anstyle::Style::new().fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Green))),
        )
        .invalid(
            anstyle::Style::new()
                .bold()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Red))),
        )
        .error(
            anstyle::Style::new()
                .bold()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Red))),
        )
        .valid(
            anstyle::Style::new()
                .bold()
                .underline()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Green))),
        )
        .placeholder(
            anstyle::Style::new().fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::White))),
        )
}

#[derive(Debug, Parser, PartialEq, Eq)]
#[command(
    styles = get_styles(),
    name = "neocmakelsp",
    about="CMake Lsp implementation based on Tower and Tree-sitter", 
    long_about = None,
    author = "Cris",
    version=LSP_VERSION
)]
pub enum NeocmakeCli {
    #[command(long_flag = "stdio", about = "run with stdio")]
    Stdio,
    #[command(long_flag = "tcp", about = "run with tcp")]
    Tcp {
        #[arg(long, value_name = "port")]
        port: Option<u16>,
    },
    #[command(long_flag = "search", short_flag = 'S', about = "search the packages")]
    Search {
        #[arg(required = true)]
        package: String,
        #[arg(value_name = "tojson", short = 'j')]
        tojson: bool,
    },
    #[command(long_flag = "format", short_flag = 'F', about = "Format the file")]
    Format {
        #[arg(required = true)]
        format_paths: Vec<String>,
        #[arg(value_name = "override", long = "override", short = 'o')]
        hasoverride: bool,
    },
    #[command(long_flag = "tree", short_flag = 'T', about = "show the file tree")]
    Tree {
        #[arg(required = true)]
        tree_path: PathBuf,
        #[arg(value_name = "tojson", short = 'j')]
        tojson: bool,
    },
    #[command(long_flag = "generate", about = "generate the completion")]
    GenCompletions {
        // If provided, outputs the completion file for given shell
        #[arg(value_enum, required = true)]
        shell: Shell,
    },
}

#[test]
fn test_claps() {
    use super::NeocmakeCli;
    use clap::{CommandFactory, FromArgMatches};
    let mut args =
        NeocmakeCli::command().get_matches_from(vec!["neocmakelsp", "format", "-o", "a", "b"]);

    let cli = NeocmakeCli::from_arg_matches_mut(&mut args).unwrap();
    if let NeocmakeCli::Format {
        format_paths,
        hasoverride: true,
    } = cli
    {
        assert_eq!(format_paths, vec!["a".to_string(), "b".to_string()]);
    } else {
        panic!("test format failed");
    }

    let mut args =
        NeocmakeCli::command().get_matches_from(vec!["neocmakelsp", "search", "-j", "dde"]);

    let cli = NeocmakeCli::from_arg_matches_mut(&mut args).unwrap();
    if let NeocmakeCli::Search {
        package,
        tojson: true,
    } = cli
    {
        assert_eq!(package, "dde".to_string());
    } else {
        panic!("test package failed");
    }

    let mut args =
        NeocmakeCli::command().get_matches_from(vec!["neocmakelsp", "tcp", "--port", "2012"]);

    let cli = NeocmakeCli::from_arg_matches_mut(&mut args).unwrap();
    assert_eq!(cli, NeocmakeCli::Tcp { port: Some(2012) });
}
