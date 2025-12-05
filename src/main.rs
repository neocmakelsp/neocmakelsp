mod ast;
mod cli;
mod complete;
mod config;
mod consts;
mod document;
mod document_link;
mod fileapi;
mod filewatcher;
mod formatting;
mod gammar;
mod hover;
mod jump;
mod languageserver;
mod quick_fix;
mod scansubs;
mod search;
mod semantic_token;
mod treesitter_nodetypes;
mod utils;

use std::net::Ipv4Addr;
use std::path::Path;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser};
use ignore::Walk;
use ini::Ini;
use tokio::net::TcpListener;
use tower_lsp::lsp_types::Uri;
use tower_lsp::{LspService, Server};
use treesitter_nodetypes as CMakeNodeKinds;

use crate::cli::{Cli, Command};
use crate::formatting::format_file;
use crate::languageserver::Backend;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EditConfigSetting {
    use_space: bool,
    indent_size: u32,
    insert_final_newline: bool,
}

impl Default for EditConfigSetting {
    fn default() -> Self {
        Self {
            use_space: true,
            indent_size: 2,
            insert_final_newline: false,
        }
    }
}

fn editconfig_setting() -> Option<EditConfigSetting> {
    let editconfig_path = std::path::Path::new(".editorconfig");
    if !editconfig_path.exists() {
        return None;
    }
    editconfig_setting_read(editconfig_path)
}

fn editconfig_setting_read<P: AsRef<Path>>(editconfig_path: P) -> Option<EditConfigSetting> {
    let conf = Ini::load_from_file(editconfig_path).unwrap();

    let cmakesession = conf.section(Some("CMakeLists.txt"))?;

    let indent_style = cmakesession.get("indent_style").unwrap_or("space");
    let use_space = indent_style == "space";

    let insert_final_newline =
        cmakesession.get("insert_final_newline").unwrap_or("false") == "true";

    let indent_size = cmakesession.get("indent_size").unwrap_or("2");
    let indent_size: u32 = if use_space {
        indent_size.parse::<u32>().unwrap_or(2)
    } else {
        1
    };

    Some(EditConfigSetting {
        use_space,
        indent_size,
        insert_final_newline,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    clap_complete::CompleteEnv::with_factory(Cli::command).complete();

    let args = Cli::parse();

    let log = tracing_subscriber::fmt();
    if matches!(args.command, Command::Stdio) {
        // NOTE: `stdio` is used for the language server protocol, so we need to log to `stderr`.
        // Most editors can't handle ANSI escape codes in their logfiles.
        log.with_writer(std::io::stderr).with_ansi(false).init();
    } else {
        log.init();
    }

    match args.command {
        Command::Stdio => {
            let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
            let (service, socket) = LspService::new(Backend::new);
            Server::new(stdin, stdout, socket).serve(service).await;
        }
        Command::Tcp { port } => {
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await?;
            let (stream, _) = listener.accept().await?;
            let (read, write) = tokio::io::split(stream);
            let (service, socket) = LspService::new(Backend::new);
            Server::new(read, write, socket).serve(service).await;
        }
        Command::Format {
            files: paths,
            inplace,
        } => {
            let EditConfigSetting {
                use_space,
                indent_size,
                insert_final_newline,
            } = editconfig_setting().unwrap_or_default();

            for path in paths {
                if !path.exists() {
                    tracing::warn!("Failed to format '{}': path doesn't exist", path.display());
                    continue;
                }
                if path.is_file() {
                    format_file(&path, inplace, use_space, indent_size, insert_final_newline)?;
                } else if path.is_dir() {
                    for entry in Walk::new(path).flatten() {
                        let path = entry.path();
                        if path.is_file()
                            && (path
                                .file_name()
                                .is_some_and(|name| name == "CMakeLists.txt")
                                || path.extension().is_some_and(|ext| ext == "cmake"))
                        {
                            format_file(
                                path,
                                inplace,
                                use_space,
                                indent_size,
                                insert_final_newline,
                            )?;
                        }
                        // FIXME: Does this ignore recursive directories??
                    }
                }
            }
        }
        Command::Search { module, json } => {
            if json {
                println!("{}", search::search_result_tojson(&module)?);
            } else {
                println!("{}", search::search_result(&module)?);
            }
        }
        Command::Tree { path, json } => {
            // If `path` is a directory try to resolve a CMakeLists.txt file.
            let path = if path.is_dir() {
                path.read_dir()?
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .find(|path| {
                        path.file_name()
                            .is_some_and(|name| name == "CMakeLists.txt")
                    })
                    .context(format!(
                        "Failed to find 'CMakeLists.txt' in {}",
                        path.display()
                    ))?
            } else {
                path
            };
            match scansubs::get_treedir(&path) {
                Some(tree) => {
                    if json {
                        println!("{}", serde_json::to_string_pretty(&tree)?);
                    } else {
                        print!("{tree}");
                    }
                }
                None => println!("Nothing found"),
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod editorconfig_test {
    use std::io::prelude::*;

    use tempfile::NamedTempFile;

    use super::{EditConfigSetting, editconfig_setting_read};
    #[test]
    fn tst_editconfig_tab() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = r#"
root = true

[*.{cmake}]
indent_style = tab
indent_size = 2

[CMakeLists.txt]
indent_style = tab
indent_size = 2

[*.{lua}]
indent_style = space
indent_size = 4
"#;
        writeln!(temp_file, "{}", content).unwrap();

        assert_eq!(
            editconfig_setting_read(temp_file),
            Some(EditConfigSetting {
                use_space: false,
                indent_size: 1,
                insert_final_newline: false
            })
        );
    }
    #[test]
    fn tst_editconfig_space() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = r#"
root = true

[CMakeLists.txt]
indent_style = space
indent_size = 2

[*.{lua}]
indent_style = space
indent_size = 4
"#;
        writeln!(temp_file, "{}", content).unwrap();

        assert_eq!(
            editconfig_setting_read(temp_file),
            Some(EditConfigSetting {
                use_space: true,
                indent_size: 2,
                insert_final_newline: false
            })
        );
    }
    #[test]
    fn tst_editconfig_lastline() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = r#"
root = true

[CMakeLists.txt]
indent_style = space
indent_size = 2
insert_final_newline = true

[*.{lua}]
indent_style = space
indent_size = 4
"#;
        writeln!(temp_file, "{}", content).unwrap();

        assert_eq!(
            editconfig_setting_read(temp_file),
            Some(EditConfigSetting {
                use_space: true,
                indent_size: 2,
                insert_final_newline: true
            })
        );
    }
}
