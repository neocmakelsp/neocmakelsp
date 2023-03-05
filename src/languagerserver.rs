use super::Backend;
use crate::ast;
use crate::complete;
use crate::filewatcher;
use crate::formatting::getformat;
use crate::gammar::checkerror;
use crate::jump;
use crate::utils::treehelper;
use serde_json::Value;
use std::path::Path;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::LanguageServer;
use tree_sitter::Parser;
impl Backend {
    async fn publish_diagnostics(&self, uri: Url, context: String) {
        let mut parse = Parser::new();
        parse.set_language(tree_sitter_cmake::language()).unwrap();
        let thetree = parse.parse(&context, None);
        if let Some(tree) = thetree {
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
    }
    async fn update_diagnostics(&self) {
        let storemap = self.buffers.lock().await;
        for (uri, context) in storemap.iter() {
            self.publish_diagnostics(uri.clone(), context.to_string())
                .await;
        }
    }
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
        self.update_diagnostics().await;
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
        let mut storemap = self.buffers.lock().await;
        storemap.insert(uri.clone(), context.clone());
        self.publish_diagnostics(uri, context).await;
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
