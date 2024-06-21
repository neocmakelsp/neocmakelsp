use std::path::PathBuf;

use clap::{arg, Parser};

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
        format_path: String,
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
}

