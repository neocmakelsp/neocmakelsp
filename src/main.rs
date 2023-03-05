use formatting::getformat;
use serde_json::Value;
use std::io::prelude::*;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
//use std::process::Command;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tree_sitter::Parser;
//use tree_sitter::Point;
use clap::{arg, Arg, ArgAction, Command};
use std::collections::HashMap;
use tokio::net::TcpListener;
mod ast;
mod complete;
mod filewatcher;
mod formatting;
mod gammar;
mod jump;
mod scansubs;
mod search;
mod utils;
use gammar::checkerror;
use utils::treehelper;

/// Beckend
#[derive(Debug)]
struct Backend {
    /// client
    client: Client,
    /// Storage the message of buffers
    buffers: Arc<Mutex<HashMap<lsp_types::Url, String>>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, inital: InitializeParams) -> Result<InitializeResult> {
        if let Some(workspace) = inital.capabilities.workspace {
            if let Some(watch_file) = workspace.did_change_watched_files {
                if let (Some(true), Some(true)) = (
                    watch_file.dynamic_registration,
                    watch_file.relative_pattern_support,
                ) {
                    if let Some(uri) = inital.root_uri {
                        let path = std::path::Path::new(uri.path())
                            .join("build")
                            .join("CMakeCache.txt");
                        if path.exists() {
                            filewatcher::refresh_error_packages(path);
                        }
                    }
                }
            }
        }
        Ok(InitializeResult {
            server_info: None,
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: None,
                    work_done_progress_options: Default::default(),
                    all_commit_characters: None,
                    completion_item: None,
                }),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec!["dummy.do_something".to_string()],
                    work_done_progress_options: Default::default(),
                }),
                document_symbol_provider: Some(OneOf::Left(true)),
                definition_provider: Some(OneOf::Left(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    file_operations: None,
                }),
                references_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        let cachefilechangeparms = DidChangeWatchedFilesRegistrationOptions {
            watchers: vec![FileSystemWatcher {
                glob_pattern: GlobPattern::String("build/CMakeCache.txt".to_string()),
                kind: Some(lsp_types::WatchKind::all()),
            }],
        };

        let cmakecache_watcher = Registration {
            id: "CMakeCacheWatcher".to_string(),
            method: "workspace/didChangeWatchedFiles".to_string(),
            register_options: Some(serde_json::to_value(cachefilechangeparms).unwrap()),
        };

        self.client
            .register_capability(vec![cmakecache_watcher])
            .await
            .unwrap();

        self.client
            .log_message(MessageType::INFO, "initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_change_workspace_folders(&self, _: DidChangeWorkspaceFoldersParams) {
        self.client
            .log_message(MessageType::INFO, "workspace folders changed!")
            .await;
    }

    async fn did_change_configuration(&self, _: DidChangeConfigurationParams) {
        self.client
            .log_message(MessageType::INFO, "configuration changed!")
            .await;
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        tracing::info!("CMakeCache changed");
        for change in params.changes {
            if let FileChangeType::DELETED = change.typ {
                filewatcher::clear_error_packages();
            } else {
                let path = change.uri.path();
                filewatcher::refresh_error_packages(path);
            }
        }
        self.client
            .log_message(MessageType::INFO, "watched files have changed!")
            .await;
    }

    async fn execute_command(&self, _: ExecuteCommandParams) -> Result<Option<Value>> {
        self.client
            .log_message(MessageType::INFO, "command executed!")
            .await;

        match self.client.apply_edit(WorkspaceEdit::default()).await {
            Ok(res) if res.applied => self.client.log_message(MessageType::INFO, "applied").await,
            Ok(_) => self.client.log_message(MessageType::INFO, "rejected").await,
            Err(err) => self.client.log_message(MessageType::ERROR, err).await,
        }

        Ok(None)
    }

    async fn did_open(&self, input: DidOpenTextDocumentParams) {
        let mut parse = Parser::new();
        parse.set_language(tree_sitter_cmake::language()).unwrap();
        let uri = input.text_document.uri.clone();
        let context = input.text_document.text.clone();
        let mut storemap = self.buffers.lock().await;
        storemap.entry(uri.clone()).or_insert(context);
        let source = input.text_document.text.clone();
        let thetree = parse.parse(source.clone(), None);
        if let Some(tree) = thetree {
            let gammererror = checkerror(Path::new(uri.path()), &source, tree.root_node());
            if let Some(diagnoses) = gammererror {
                let mut pusheddiagnoses = vec![];
                for (start, end, message, severity) in diagnoses.inner {
                    let pointx = lsp_types::Position::new(start.row as u32, start.column as u32);
                    let pointy = lsp_types::Position::new(end.row as u32, end.column as u32);
                    let range = Range {
                        start: pointx,
                        end: pointy,
                    };
                    let diagnose = Diagnostic {
                        range,
                        severity,
                        code: None,
                        code_description: None,
                        source: None,
                        message,
                        related_information: None,
                        tags: None,
                        data: None,
                    };
                    pusheddiagnoses.push(diagnose);
                }
                self.client
                    .publish_diagnostics(input.text_document.uri.clone(), pusheddiagnoses, Some(1))
                    .await;
            }
        }
        self.client
            .log_message(MessageType::INFO, "file opened!")
            .await;
    }

    async fn did_change(&self, input: DidChangeTextDocumentParams) {
        // create a parse
        let mut parse = Parser::new();
        let uri = input.text_document.uri.clone();
        let context = input.content_changes[0].text.clone();
        let mut storemap = self.buffers.lock().await;
        storemap.insert(uri.clone(), context);
        parse.set_language(tree_sitter_cmake::language()).unwrap();

        let source = input.content_changes[0].text.clone();
        let thetree = parse.parse(source.clone(), None);
        if let Some(tree) = thetree {
            let gammererror = checkerror(Path::new(&uri.path()), &source, tree.root_node());
            if let Some(diagnoses) = gammererror {
                let mut pusheddiagnoses = vec![];
                for (start, end, message, severity) in diagnoses.inner {
                    let pointx = lsp_types::Position::new(start.row as u32, start.column as u32);
                    let pointy = lsp_types::Position::new(end.row as u32, end.column as u32);

                    let range = Range {
                        start: pointx,
                        end: pointy,
                    };
                    let diagnose = Diagnostic {
                        range,
                        severity,
                        code: None,
                        code_description: None,
                        source: None,
                        message,
                        related_information: None,
                        tags: None,
                        data: None,
                    };
                    pusheddiagnoses.push(diagnose);
                }
                self.client
                    .publish_diagnostics(input.text_document.uri.clone(), pusheddiagnoses, Some(1))
                    .await;
            } else {
                self.client
                    .publish_diagnostics(input.text_document.uri.clone(), vec![], None)
                    .await;
                //self.client.semantic_tokens_refresh().await.unwrap();
            }
        }
        self.client
            .log_message(MessageType::INFO, &format!("{input:?}"))
            .await;
    }

    async fn did_save(&self, _: DidSaveTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file saved!")
            .await;
    }
    async fn signature_help(&self, _: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        Ok(Some(SignatureHelp {
            signatures: vec![SignatureInformation {
                label: "Test".to_string(),
                documentation: None,
                parameters: None,
                active_parameter: None,
            }],
            active_signature: None,
            active_parameter: None,
        }))
    }
    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let position = params.text_document_position_params.position;
        let uri = params.text_document_position_params.text_document.uri;
        let storemap = self.buffers.lock().await;
        self.client.log_message(MessageType::INFO, "Hovered!").await;
        //notify_send("test", Type::Error);
        match storemap.get(&uri) {
            Some(context) => {
                let mut parse = Parser::new();
                parse.set_language(tree_sitter_cmake::language()).unwrap();
                let thetree = parse.parse(context.clone(), None);
                let tree = thetree.unwrap();
                let output = treehelper::get_cmake_doc(position, tree.root_node(), context);
                match output {
                    Some(context) => Ok(Some(Hover {
                        contents: HoverContents::Scalar(MarkedString::String(context)),
                        range: Some(Range {
                            start: position,
                            end: position,
                        }),
                    })),
                    None => Ok(None),
                }
                //notify_send(context, Type::Error);
                //Ok(None)
            }
            None => Ok(None),
        }
    }

    async fn formatting(&self, input: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        self.client
            .log_message(MessageType::INFO, "formating")
            .await;
        let uri = input.text_document.uri;
        let storemap = self.buffers.lock().await;
        tracing::info!(input.options.insert_spaces);
        let space_line = if input.options.insert_spaces {
            input.options.tab_size
        } else {
            1
        };
        match storemap.get(&uri) {
            Some(context) => Ok(getformat(
                context,
                &self.client,
                space_line,
                input.options.insert_spaces,
            )
            .await),
            None => Ok(None),
        }
    }

    async fn did_close(&self, _: DidCloseTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file closed!")
            .await;
        //notify_send("file closed", Type::Info);
    }
    async fn completion(&self, input: CompletionParams) -> Result<Option<CompletionResponse>> {
        self.client.log_message(MessageType::INFO, "Complete").await;
        let location = input.text_document_position.position;
        if input.context.is_some() {
            let uri = input.text_document_position.text_document.uri;
            let storemap = self.buffers.lock().await;
            //notify_send("test", Type::Error);
            match storemap.get(&uri) {
                Some(context) => {
                    Ok(complete::getcoplete(context, location, &self.client, uri.path()).await)
                }
                None => Ok(None),
            }
        } else {
            Ok(None)
        }
    }
    async fn references(&self, input: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = input.text_document_position.text_document.uri;
        //println!("{:?}", uri);
        let location = input.text_document_position.position;
        let storemap = self.buffers.lock().await;
        match storemap.get(&uri) {
            Some(context) => {
                let mut parse = Parser::new();
                parse.set_language(tree_sitter_cmake::language()).unwrap();
                //notify_send(context, Type::Error);
                Ok(jump::godef(location, context, uri.path().to_string(), &self.client).await)
            }
            None => Ok(None),
        }
    }
    async fn goto_definition(
        &self,
        input: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = input.text_document_position_params.text_document.uri;
        //println!("{:?}", uri);
        let location = input.text_document_position_params.position;
        let storemap = self.buffers.lock().await;
        match storemap.get(&uri) {
            Some(context) => {
                let mut parse = Parser::new();
                parse.set_language(tree_sitter_cmake::language()).unwrap();
                let thetree = parse.parse(context.clone(), None);
                let tree = thetree.unwrap();
                let origin_selection_range =
                    treehelper::get_position_range(location, tree.root_node());

                //notify_send(context, Type::Error);
                match jump::godef(location, context, uri.path().to_string(), &self.client).await {
                    Some(range) => Ok(Some(GotoDefinitionResponse::Link({
                        range
                            .iter()
                            .filter(|input| match origin_selection_range {
                                Some(origin) => origin != input.range,
                                None => true,
                            })
                            .map(|range| LocationLink {
                                origin_selection_range,
                                target_uri: range.uri.clone(),
                                target_range: range.range,
                                target_selection_range: range.range,
                            })
                            .collect()
                    }))),
                    None => Ok(None),
                }

                //Ok(None)
            }
            None => Ok(None),
        }
        //Ok(None)
    }
    // TODO ? Why cannot get it?
    async fn document_symbol(
        &self,
        input: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = input.text_document.uri.clone();
        let storemap = self.buffers.lock().await;
        //notify_send("test", Type::Error);
        match storemap.get(&uri) {
            Some(context) => {
                let mut parse = Parser::new();
                parse.set_language(tree_sitter_cmake::language()).unwrap();
                let thetree = parse.parse(context.clone(), None);
                let tree = thetree.unwrap();
                //notify_send(context, Type::Error);
                //Ok(None)
                Ok(ast::getast(tree.root_node(), context))
            }
            None => Ok(None),
        }
    }
}

#[tokio::main]
async fn main() {
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    let matches = Command::new("neocmakelsp")
        .about("neo lsp for cmake")
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
                .arg(arg!(<PATH> ... "path to format").value_parser(clap::value_parser!(PathBuf))),
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
                .expect("required one pacakge");
            if sub_matches.get_flag("tojson") {
                println!("{}", search::search_result_tojson(packagename));
            } else {
                println!("{}", search::search_result(packagename));
            }
        }
        Some(("format", sub_matches)) => {
            let path = sub_matches
                .get_one::<PathBuf>("PATH")
                .expect("Cannot get path");
            let mut file = std::fs::File::open(path).unwrap();
            let mut buf = String::new();
            file.read_to_string(&mut buf).unwrap();
            let mut parse = tree_sitter::Parser::new();
            parse.set_language(tree_sitter_cmake::language()).unwrap();
            let tree = parse.parse(&buf, None).unwrap();
            match formatting::get_format_cli(tree.root_node(), &buf) {
                Some(context) => println!("{context}"),
                None => println!("There is error in File"),
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
                buffers: Arc::new(Mutex::new(HashMap::new())),
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
                buffers: Arc::new(Mutex::new(HashMap::new())),
            });
            Server::new(read, write, socket).serve(service).await;
        }
        _ => unimplemented!(),
    }
}
