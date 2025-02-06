use std::io::prelude::*;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};

use clap::Parser;
use ini::Ini;
use tower_lsp::{Client, LspService, Server};
mod treesitter_nodetypes;

use tokio::net::TcpListener;
use treesitter_nodetypes as CMakeNodeKinds;
mod ast;
mod clapargs;
mod complete;
mod config;
mod consts;
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
mod shellcomplete;
mod utils;
use clapargs::NeocmakeCli;
use std::sync::OnceLock;
use tower_lsp::lsp_types::Url;

#[derive(Debug)]
struct BackendInitInfo {
    pub scan_cmake_in_package: bool,
    pub enable_lint: bool,
}

impl Default for BackendInitInfo {
    fn default() -> Self {
        Self {
            scan_cmake_in_package: true,
            enable_lint: true,
        }
    }
}

/// Beckend
#[derive(Debug)]
struct Backend {
    /// client
    client: Client,
    /// Storage the message of buffers
    init_info: OnceLock<BackendInitInfo>,
    root_path: OnceLock<Option<PathBuf>>,
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            init_info: OnceLock::new(),
            root_path: OnceLock::new(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
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
async fn main() {
    let log = tracing_subscriber::fmt();

    let args = NeocmakeCli::parse();
    if matches!(args, NeocmakeCli::Stdio) {
        // NOTE: `stdout` should be retained when sending rpc messages.
        // Also, most editors can't handle the ANSI escape codes properly in their log capture.
        log.with_writer(std::io::stderr).with_ansi(false).init();
    } else {
        log.init();
    }
    match args {
        NeocmakeCli::Stdio => {
            let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
            let (service, socket) = LspService::new(Backend::new);
            Server::new(stdin, stdout, socket).serve(service).await;
        }
        NeocmakeCli::Tcp { port } => {
            let stream = {
                if let Some(port) = port {
                    let listener = TcpListener::bind(SocketAddr::new(
                        std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                        port,
                    ))
                    .await
                    .unwrap();
                    let (stream, _) = listener.accept().await.unwrap();
                    stream
                } else {
                    let listener = TcpListener::bind(SocketAddr::new(
                        std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                        9257,
                    ))
                    .await
                    .unwrap();
                    let (stream, _) = listener.accept().await.unwrap();
                    stream
                }
            };

            let (read, write) = tokio::io::split(stream);

            let (service, socket) = LspService::new(Backend::new);
            Server::new(read, write, socket).serve(service).await;
        }
        NeocmakeCli::Tree { tree_path, tojson } => {
            match scansubs::get_treedir(&tree_path) {
                Some(tree) => {
                    if tojson {
                        println!("{}", serde_json::to_string(&tree).unwrap())
                    } else {
                        println!("{tree}")
                    }
                }
                None => println!("Nothing find"),
            };
        }
        NeocmakeCli::Search { package, tojson } => {
            if tojson {
                println!("{}", search::search_result_tojson(package.as_str()));
            } else {
                println!("{}", search::search_result(package.as_str()));
            }
        }
        NeocmakeCli::Format {
            format_paths,
            hasoverride,
        } => {
            use std::path::Path;

            use ignore::Walk;
            let EditConfigSetting {
                use_space,
                indent_size,
                insert_final_newline,
            } = editconfig_setting().unwrap_or_default();
            let format_file = |format_file: &Path| {
                let mut file = match std::fs::OpenOptions::new()
                    .read(true)
                    .write(hasoverride)
                    .open(format_file)
                {
                    Ok(file) => file,
                    Err(e) => {
                        println!("cannot read file {} :{e}", format_file.display());
                        return;
                    }
                };
                let mut buf = String::new();
                if let Err(e) = file.read_to_string(&mut buf) {
                    println!("cannot read {} : error {}", format_file.display(), e);
                    return;
                }
                match formatting::get_format_cli(&buf, indent_size, use_space, insert_final_newline)
                {
                    Some(context) => {
                        if hasoverride {
                            if let Err(e) = file.set_len(0) {
                                println!("Cannot clear the file: {e}");
                            };
                            if let Err(e) = file.seek(std::io::SeekFrom::End(0)) {
                                println!("Cannot jump to end: {e}");
                            };
                            let Ok(_) = file.write_all(context.as_bytes()) else {
                                println!("cannot write in {}", format_file.display());
                                return;
                            };
                            let _ = file.flush();
                        } else {
                            println!("{context}")
                        }
                    }
                    None => {
                        println!("There is error in file: {}", format_file.display());
                    }
                }
            };

            for format_path in format_paths {
                let toformatpath = Path::new(format_path.as_str());
                if !toformatpath.exists() {
                    continue;
                }
                if toformatpath.is_file() {
                    format_file(toformatpath);
                } else {
                    for results in Walk::new(&format_path).flatten() {
                        let file_path = results.path();
                        if file_path.is_dir() {
                            continue;
                        }
                        if file_path.ends_with("CMakeLists.txt")
                            || file_path.extension().is_some_and(|ex| ex == "cmake")
                        {
                            format_file(file_path);
                        }
                    }
                }
            }
        }
        NeocmakeCli::GenCompletion { shell } => shellcomplete::generate_shell_completion(shell),
    }
}

#[cfg(test)]
mod editorconfig_test {
    use std::io::prelude::*;

    use tempfile::NamedTempFile;

    use super::{editconfig_setting_read, EditConfigSetting};
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
        )
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
        )
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
        )
    }
}
