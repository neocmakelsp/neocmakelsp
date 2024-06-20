use std::io::prelude::*;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use consts::TREESITTER_CMAKE_LANGUAGE;
//use std::process::Command;
use ini::Ini;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LspService, Server};
//use tree_sitter::Point;
use clap::{arg, Arg, ArgAction, Command};

use ignore::gitignore::{Gitignore, GitignoreBuilder};
// color
use nu_ansi_term::Color::LightYellow;

use tokio::net::TcpListener;
mod ast;
mod complete;
mod config;
mod filewatcher;
mod formatting;
mod gammar;
mod jump;
mod languageserver;
mod scansubs;
mod search;
mod semantic_token;
mod consts;
mod utils;

#[derive(Debug)]
struct BackendInitInfo {
    pub scan_cmake_in_package: bool,
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

fn gitignore() -> Option<Gitignore> {
    let gitignore = std::path::Path::new(".gitignore");
    if !gitignore.exists() {
        return None;
    }
    let mut builder = GitignoreBuilder::new(std::env::current_dir().ok()?);
    builder.add(gitignore);
    builder.build().ok()
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
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    let matches = Command::new("neocmakelsp")
        .about(
            LightYellow
                .paint("CMake LSP implementation based on Tower and Tree-sitter")
                .to_string(),
        )
        .version(VERSION)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .author("Cris")
        .subcommand(
            Command::new("stdio")
                .long_flag("stdio")
                .about("run with stdio"),
        )
        .subcommand(
            Command::new("tcp")
                .long_flag("tcp")
                .about("run with tcp")
                .arg(
                    Arg::new("port")
                        .long("port")
                        .short('P')
                        .help("listen to port"),
                ),
        )
        .subcommand(
            Command::new("search")
                .long_flag("search")
                .short_flag('S')
                .about("Search packages")
                .arg(arg!(<Package> ... "Packages"))
                .arg(
                    Arg::new("tojson")
                        .long("tojson")
                        .short('j')
                        .action(ArgAction::SetTrue)
                        .help("tojson"),
                ),
        )
        .subcommand(
            Command::new("format")
                .long_flag("format")
                .short_flag('F')
                .about("format the file")
                .arg(
                    arg!(<FormatPath> ... "file or folder to format")
                        .value_parser(clap::value_parser!(String)),
                )
                .arg(
                    Arg::new("override")
                        .long("override")
                        .short('o')
                        .action(ArgAction::SetTrue)
                        .help("override"),
                ),
        )
        .subcommand(
            Command::new("tree")
                .long_flag("tree")
                .short_flag('T')
                .about("Tree the file")
                .arg(arg!(<PATH> ... "tree").value_parser(clap::value_parser!(PathBuf)))
                .arg(
                    Arg::new("tojson")
                        .long("tojson")
                        .short('j')
                        .action(ArgAction::SetTrue)
                        .help("tojson"),
                ),
        )
        .get_matches();
    match matches.subcommand() {
        Some(("search", sub_matches)) => {
            let packagename = sub_matches
                .get_one::<String>("Package")
                .expect("required one package");
            if sub_matches.get_flag("tojson") {
                println!("{}", search::search_result_tojson(packagename));
            } else {
                println!("{}", search::search_result(packagename));
            }
        }
        Some(("format", sub_matches)) => {
            let filepath = sub_matches
                .get_one::<String>("FormatPath")
                .expect("Cannot get globpattern");
            let hasoverride = sub_matches.get_flag("override");
            let (use_space, spacelen) = editconfig_setting().unwrap_or((true, 2));
            let ignorepatterns = gitignore();

            let formatpattern = |pattern: &str| {
                for filepath in glob::glob(pattern)
                    .unwrap_or_else(|_| panic!("error pattern"))
                    .flatten()
                {
                    if let Some(ref ignorepatterns) = ignorepatterns {
                        if ignorepatterns.matched(&filepath, false).is_ignore() {
                            continue;
                        }
                    }

                    let mut file = match std::fs::OpenOptions::new()
                        .read(true)
                        .write(hasoverride)
                        .open(&filepath)
                    {
                        Ok(file) => file,
                        Err(e) => {
                            println!("cannot read file {} :{e}", filepath.display());
                            continue;
                        }
                    };
                    let mut buf = String::new();
                    file.read_to_string(&mut buf).unwrap();
                    let mut parse = tree_sitter::Parser::new();
                    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
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
                                    println!("cannot write in {}", filepath.display());
                                    continue;
                                };
                                let _ = file.flush();
                            } else {
                                println!("== Format of file {} is ==", filepath.display());
                                println!("{context}");
                                println!("== End ==");
                                println!();
                            }
                        }
                        None => {
                            println!("There is error in file: {}", filepath.display());
                        }
                    }
                }
            };
            let toformatpath = std::path::Path::new(filepath);
            if toformatpath.exists() {
                if toformatpath.is_file() {
                    let mut file = match std::fs::OpenOptions::new()
                        .read(true)
                        .write(hasoverride)
                        .open(filepath)
                    {
                        Ok(file) => file,
                        Err(e) => {
                            println!("cannot read file {} :{e}", filepath);
                            return;
                        }
                    };
                    let mut buf = String::new();
                    file.read_to_string(&mut buf).unwrap();
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
                                    println!("cannot write in {}", filepath);
                                    return;
                                };
                                let _ = file.flush();
                            } else {
                                println!("{context}")
                            }
                        }
                        None => {
                            println!("There is error in file: {}", filepath);
                        }
                    }
                } else {
                    formatpattern(&format!("./{}/**/*.cmake", filepath));
                    formatpattern(&format!("./{}/**/CMakeLists.txt", filepath));
                }
            }
        }
        Some(("tree", sub_matches)) => {
            let path = sub_matches
                .get_one::<PathBuf>("PATH")
                .expect("Cannot get path");
            match scansubs::get_treedir(path) {
                Some(tree) => {
                    if sub_matches.get_flag("tojson") {
                        println!("{}", serde_json::to_string(&tree).unwrap())
                    } else {
                        println!("{tree}")
                    }
                }
                None => println!("Nothing find"),
            };
        }
        Some(("stdio", _)) => {
            tracing_subscriber::fmt().init();
            let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
            let (service, socket) = LspService::new(|client| Backend {
                client,
                init_info: Arc::new(Mutex::new(BackendInitInfo {
                    scan_cmake_in_package: true,
                })),
                root_path: Arc::new(Mutex::new(None)),
            });
            Server::new(stdin, stdout, socket).serve(service).await;
        }
        Some(("tcp", sync_matches)) => {
            #[cfg(feature = "runtime-agnostic")]
            use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
            tracing_subscriber::fmt().init();
            let stream = {
                if sync_matches.contains_id("port") {
                    let port = sync_matches.get_one::<String>("port").expect("error");
                    let port: u16 = port.parse().unwrap();
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
            #[cfg(feature = "runtime-agnostic")]
            let (read, write) = (read.compat(), write.compat_write());

            let (service, socket) = LspService::new(|client| Backend {
                client,
                init_info: Arc::new(Mutex::new(BackendInitInfo {
                    scan_cmake_in_package: true,
                })),
                root_path: Arc::new(Mutex::new(None)),
            });
            Server::new(read, write, socket).serve(service).await;
        }
        _ => unimplemented!(),
    }
}
