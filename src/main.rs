use std::io::prelude::*;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
//use std::process::Command;
use ini::Ini;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::{Client, LspService, Server};
//use tree_sitter::Point;
use clap::Parser;

use tokio::net::TcpListener;
mod ast;
mod clapargs;
mod complete;
mod config;
mod consts;
mod fileapi;
mod filewatcher;
mod formatting;
mod gammar;
mod hover;
mod jump;
mod languageserver;
mod scansubs;
mod search;
mod semantic_token;
mod utils;

use clapargs::NeocmakeCli;

#[derive(Debug)]
struct BackendInitInfo {
    pub scan_cmake_in_package: bool,
    pub enable_lint: bool,
}

/// Beckend
#[derive(Debug)]
struct Backend {
    /// client
    client: Client,
    /// Storage the message of buffers
    init_info: Arc<Mutex<BackendInitInfo>>,
    root_path: Arc<Mutex<Option<PathBuf>>>,
}

fn editconfig_setting() -> Option<(bool, u32)> {
    let editconfig_path = std::path::Path::new(".editorconfig");
    if !editconfig_path.exists() {
        return None;
    }
    let conf = Ini::load_from_file(editconfig_path).unwrap();

    let cmakesession = conf.section(Some("CMakeLists.txt"))?;

    let indent_style = cmakesession.get("indent_style").unwrap_or("space");
    let use_space = indent_style == "space";
    let indent_size = cmakesession.get("indent_size").unwrap_or("2");
    let indent_size: u32 = if use_space {
        indent_size.parse::<u32>().unwrap_or(2)
    } else {
        1
    };

    Some((use_space, indent_size))
}

#[tokio::main]
async fn main() {
    let args = NeocmakeCli::parse();
    match args {
        NeocmakeCli::Stdio => {
            tracing_subscriber::fmt().init();
            let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
            let (service, socket) = LspService::new(|client| Backend {
                client,
                init_info: Arc::new(Mutex::new(BackendInitInfo {
                    scan_cmake_in_package: true,
                    enable_lint: true,
                })),
                root_path: Arc::new(Mutex::new(None)),
            });
            Server::new(stdin, stdout, socket).serve(service).await;
        }
        NeocmakeCli::Tcp { port } => {
            tracing_subscriber::fmt().init();
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

            let (service, socket) = LspService::new(|client| Backend {
                client,
                init_info: Arc::new(Mutex::new(BackendInitInfo {
                    scan_cmake_in_package: true,
                    enable_lint: true,
                })),
                root_path: Arc::new(Mutex::new(None)),
            });
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
            use ignore::Walk;
            use std::path::Path;
            let (use_space, spacelen) = editconfig_setting().unwrap_or((true, 2));
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
                match formatting::get_format_cli(&buf, spacelen, use_space) {
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
    }
}
