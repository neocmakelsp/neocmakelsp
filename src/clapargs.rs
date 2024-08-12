use std::path::PathBuf;

use clap::{arg, Parser};
use clap_complete::Shell;

const LSP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Parser, PartialEq, Eq)]
#[command(
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
    #[command(long_flag = "generate", about = "genarate the completion")]
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
