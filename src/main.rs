use serde_json::Value;
//use std::process::Command;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tree_sitter::Parser;
//use tree_sitter::Point;
use clap::{Arg, Command};
use std::collections::HashMap;
use tokio::net::{TcpListener, TcpStream};

mod ast;
mod complete;
mod gammar;
mod jump;
mod utils;
mod snippets;
mod treehelper;
use gammar::checkerror;

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
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
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
                }),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec!["dummy.do_something".to_string()],
                    work_done_progress_options: Default::default(),
                }),
                document_symbol_provider: Some(OneOf::Left(true)),
                definition_provider: Some(OneOf::Left(true)),
                // TODO
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    file_operations: None,
                }),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
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

    async fn did_change_watched_files(&self, _: DidChangeWatchedFilesParams) {
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
        storemap.entry(uri).or_insert(context);
        let thetree = parse.parse(input.text_document.text.clone(), None);
        if let Some(tree) = thetree {
            let gammererror = checkerror(tree.root_node());
            if let Some(diagnoses) = gammererror {
                let mut pusheddiagnoses = vec![];
                for (start, end) in diagnoses {
                    let pointx = lsp_types::Position::new(start.row as u32, start.column as u32);
                    let pointy = lsp_types::Position::new(end.row as u32, end.column as u32);
                    let range = Range {
                        start: pointx,
                        end: pointy,
                    };
                    let diagnose = Diagnostic {
                        range,
                        severity: None,
                        code: None,
                        code_description: None,
                        source: None,
                        message: "gammererror".to_string(),
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
        storemap.insert(uri, context);
        parse.set_language(tree_sitter_cmake::language()).unwrap();
        let thetree = parse.parse(input.content_changes[0].text.clone(), None);
        if let Some(tree) = thetree {
            let gammererror = checkerror(tree.root_node());
            if let Some(diagnoses) = gammererror {
                let mut pusheddiagnoses = vec![];
                for (start, end) in diagnoses {
                    let pointx = lsp_types::Position::new(start.row as u32, start.column as u32);
                    let pointy = lsp_types::Position::new(end.row as u32, end.column as u32);

                    let range = Range {
                        start: pointx,
                        end: pointy,
                    };
                    let diagnose = Diagnostic {
                        range,
                        severity: None,
                        code: None,
                        code_description: None,
                        source: None,
                        message: "gammererror".to_string(),
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
            .log_message(MessageType::INFO, &format!("{:?}", input))
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
    async fn did_close(&self, _: DidCloseTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file closed!")
            .await;
        //notify_send("file closed", Type::Info);
    }
    async fn completion(&self, input: CompletionParams) -> Result<Option<CompletionResponse>> {
        self.client.log_message(MessageType::INFO, "Complete").await;
        if input.context.is_some() {
            let uri = input.text_document_position.text_document.uri;
            let storemap = self.buffers.lock().await;
            //notify_send("test", Type::Error);
            match storemap.get(&uri) {
                Some(context) => {
                    let mut parse = Parser::new();
                    parse.set_language(tree_sitter_cmake::language()).unwrap();
                    let thetree = parse.parse(context.clone(), None);
                    let tree = thetree.unwrap();
                    Ok(complete::getcoplete(tree.root_node(), context))
                }
                None => Ok(None),
            }
        } else {
            Ok(None)
        }
    }
    async fn goto_definition(
        &self,
        input: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = input.text_document_position_params.text_document.uri;
        println!("{:?}", uri);
        let location = input.text_document_position_params.position;
        let storemap = self.buffers.lock().await;
        match storemap.get(&uri) {
            Some(context) => {
                let mut parse = Parser::new();
                parse.set_language(tree_sitter_cmake::language()).unwrap();
                let thetree = parse.parse(context.clone(), None);
                let tree = thetree.unwrap();
                let origin_selection_range =
                    treehelper::get_positon_range(location, tree.root_node(), context);

                //notify_send(context, Type::Error);
                //Ok(None)
                match jump::godef(location, tree.root_node(), context, uri.to_string()) {
                    Some(range) => Ok(Some(GotoDefinitionResponse::Link({
                        range
                            .iter()
                            .filter(|input| match origin_selection_range {
                                Some(origin) => origin != input.range,
                                None => true,
                            })
                            .map(|range| LocationLink {
                                origin_selection_range,
                                target_uri: Url::parse(&format!("file://{}",range.uri)).unwrap(),
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
                    Arg::new("listen")
                        .long("listen")
                        .help("listen to port"),
                ),
        )
        .get_matches();
    match matches.subcommand() {
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
                if sync_matches.contains_id("listen") {
                    let listener = TcpListener::bind("127.0.0.1:9257").await.unwrap();
                    let (stream, _) = listener.accept().await.unwrap();
                    stream
                } else {
                    TcpStream::connect("127.0.0.1:9257").await.unwrap()
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
