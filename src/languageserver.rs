mod config;

use self::config::Config;

use super::Backend;
use crate::ast;
use crate::complete;
use crate::filewatcher;
use crate::formatting::getformat;
use crate::gammar::checkerror;
use crate::jump;
use crate::scansubs;
use crate::semantic_token;
use crate::semantic_token::LEGEND_TYPE;
use crate::utils::treehelper;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types;
use tower_lsp::lsp_types::*;
use tower_lsp::LanguageServer;
use tree_sitter::Parser;

use once_cell::sync::Lazy;

pub static BUFFERS_CACHE: Lazy<Arc<Mutex<HashMap<lsp_types::Url, String>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

impl Backend {
    async fn publish_diagnostics(&self, uri: Url, context: String) {
        let mut parse = Parser::new();
        parse.set_language(&tree_sitter_cmake::language()).unwrap();
        let thetree = parse.parse(&context, None);
        let Some(tree) = thetree else {
            return;
        };
        let gammererror = checkerror(Path::new(uri.path()), &context, tree.root_node());
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
                .publish_diagnostics(uri, pusheddiagnoses, Some(1))
                .await;
        } else {
            self.client.publish_diagnostics(uri, vec![], None).await;
        }
    }
    async fn update_diagnostics(&self) {
        let storemap = BUFFERS_CACHE.lock().await;
        for (uri, context) in storemap.iter() {
            self.publish_diagnostics(uri.clone(), context.to_string())
                .await;
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, inital: InitializeParams) -> Result<InitializeResult> {
        let inital_config: Config = inital
            .initialization_options
            .and_then(|value| serde_json::from_value(value).unwrap_or(None))
            .unwrap_or_default();

        let do_format = inital_config.is_format_enabled();

        let find_cmake_in_package = inital_config.is_scan_cmake_in_package();

        let mut init_info = self.init_info.lock().await;
        init_info.scan_cmake_in_package = find_cmake_in_package;

        if let Some(workspace) = inital.capabilities.workspace {
            if let Some(watch_file) = workspace.did_change_watched_files {
                if let (Some(true), Some(true)) = (
                    watch_file.dynamic_registration,
                    watch_file.relative_pattern_support,
                ) {
                    if let Some(ref uri) = inital.root_uri {
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

        if let Some(ref uri) = inital.root_uri {
            scansubs::scan_all(uri.path()).await;
            let mut root_path = self.root_path.lock().await;
            root_path.replace(uri.path().into());
        }

        let version: String = env!("CARGO_PKG_VERSION").to_string();
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "neocmakelsp".to_string(),
                version: Some(version),
            }),
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
                document_symbol_provider: Some(OneOf::Left(true)),
                definition_provider: Some(OneOf::Left(true)),
                document_formatting_provider: if do_format {
                    Some(OneOf::Left(true))
                } else {
                    None
                },
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    file_operations: None,
                }),
                semantic_tokens_provider: if inital_config.enable_semantic_token() {
                    Some(
                        SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(
                            SemanticTokensRegistrationOptions {
                                text_document_registration_options: {
                                    TextDocumentRegistrationOptions {
                                        document_selector: Some(vec![DocumentFilter {
                                            language: Some("cmake".to_string()),
                                            scheme: Some("file".to_string()),
                                            pattern: None,
                                        }]),
                                    }
                                },
                                semantic_tokens_options: SemanticTokensOptions {
                                    work_done_progress_options: WorkDoneProgressOptions::default(),
                                    legend: SemanticTokensLegend {
                                        token_types: LEGEND_TYPE.into(),
                                        token_modifiers: vec![],
                                    },
                                    range: None,
                                    full: Some(SemanticTokensFullOptions::Bool(true)),
                                },
                                static_registration_options: StaticRegistrationOptions::default(),
                            },
                        ),
                    )
                } else {
                    None
                },
                references_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        let cachefilechangeparms = DidChangeWatchedFilesRegistrationOptions {
            watchers: vec![
                FileSystemWatcher {
                    glob_pattern: GlobPattern::String("**/CMakeCache.txt".to_string()),
                    kind: Some(lsp_types::WatchKind::all()),
                },
                FileSystemWatcher {
                    glob_pattern: GlobPattern::String("**/CMakeLists.txt".to_string()),
                    kind: Some(lsp_types::WatchKind::Create | lsp_types::WatchKind::Delete),
                },
            ],
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
        for change in params.changes {
            if let Some("CMakeLists.txt") = change.uri.path().split('/').last() {
                let Some(ref path) = *self.root_path.lock().await else {
                    continue;
                };
                scansubs::scan_all(path).await;
                continue;
            }
            tracing::info!("CMakeCache changed");
            if let FileChangeType::DELETED = change.typ {
                filewatcher::clear_error_packages();
            } else {
                let path = change.uri.path();
                filewatcher::refresh_error_packages(path);
            }
        }
        self.update_diagnostics().await;
        self.client
            .log_message(MessageType::INFO, "watched files have changed!")
            .await;
    }

    async fn did_open(&self, input: DidOpenTextDocumentParams) {
        let mut parse = Parser::new();
        parse.set_language(&tree_sitter_cmake::language()).unwrap();
        let uri = input.text_document.uri.clone();
        let context = input.text_document.text.clone();
        let mut storemap = BUFFERS_CACHE.lock().await;
        storemap.entry(uri.clone()).or_insert(context.clone());
        self.publish_diagnostics(uri, context).await;
        self.client
            .log_message(MessageType::INFO, "file opened!")
            .await;
    }

    async fn did_change(&self, input: DidChangeTextDocumentParams) {
        // create a parse
        let uri = input.text_document.uri.clone();
        let context = input.content_changes[0].text.clone();
        let mut storemap = BUFFERS_CACHE.lock().await;
        storemap.insert(uri.clone(), context.clone());
        if context.lines().count() < 500 {
            self.publish_diagnostics(uri, context).await;
        }
        self.client
            .log_message(MessageType::INFO, &format!("{input:?}"))
            .await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        let storemap = BUFFERS_CACHE.lock().await;

        let has_root = self.root_path.lock().await.is_some();
        if has_root {
            scansubs::scan_dir(uri.path()).await;
        };

        if let Some(context) = storemap.get(&uri) {
            if has_root {
                complete::update_cache(uri.path(), context).await;
            }
            self.publish_diagnostics(uri, context.to_string()).await;
        }
        self.client
            .log_message(MessageType::INFO, "file saved!")
            .await;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let position = params.text_document_position_params.position;
        let uri = params.text_document_position_params.text_document.uri;
        let storemap = BUFFERS_CACHE.lock().await;
        self.client.log_message(MessageType::INFO, "Hovered!").await;
        //notify_send("test", Type::Error);
        match storemap.get(&uri) {
            Some(context) => {
                let mut parse = Parser::new();
                parse.set_language(&tree_sitter_cmake::language()).unwrap();
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
            .log_message(
                MessageType::INFO,
                format!("formating, space is {}", input.options.insert_spaces),
            )
            .await;
        let uri = input.text_document.uri;
        let storemap = BUFFERS_CACHE.lock().await;
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

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.client
            .log_message(
                MessageType::INFO,
                format!("file {:?} closed!", params.text_document.uri),
            )
            .await;
        //notify_send("file closed", Type::Info);
    }
    async fn completion(&self, input: CompletionParams) -> Result<Option<CompletionResponse>> {
        self.client.log_message(MessageType::INFO, "Complete").await;
        let location = input.text_document_position.position;
        let uri = input.text_document_position.text_document.uri;
        let storemap = BUFFERS_CACHE.lock().await;
        let urlconent = storemap.get(&uri).cloned();
        drop(storemap);
        match urlconent {
            Some(context) => Ok(complete::getcomplete(
                &context,
                location,
                &self.client,
                uri.path(),
                self.init_info.lock().await.scan_cmake_in_package,
            )
            .await),
            None => Ok(None),
        }
    }
    async fn references(&self, input: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = input.text_document_position.text_document.uri;
        //println!("{:?}", uri);
        let location = input.text_document_position.position;
        let storemap = BUFFERS_CACHE.lock().await;
        match storemap.get(&uri) {
            Some(context) => {
                let mut parse = Parser::new();
                parse.set_language(&tree_sitter_cmake::language()).unwrap();
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
        let location = input.text_document_position_params.position;
        let storemap = BUFFERS_CACHE.lock().await;
        match storemap.get(&uri) {
            Some(context) => {
                let mut parse = Parser::new();
                parse.set_language(&tree_sitter_cmake::language()).unwrap();
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
    async fn document_symbol(
        &self,
        input: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = input.text_document.uri.clone();
        let storemap = BUFFERS_CACHE.lock().await;
        match storemap.get(&uri) {
            Some(context) => Ok(ast::getast(&self.client, context).await),
            None => Ok(None),
        }
    }
    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri.clone();
        self.client
            .log_message(MessageType::LOG, "semantic_token_full")
            .await;
        let storemap = BUFFERS_CACHE.lock().await;
        match storemap.get(&uri) {
            Some(context) => Ok(semantic_token::semantic_token(&self.client, context).await),
            None => Ok(None),
        }
    }
}
