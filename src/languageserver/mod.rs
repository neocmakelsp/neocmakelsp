mod config;
mod rename;

use std::path::{Component, Path, PathBuf};
use std::process::exit;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{OnceLock, RwLock};

use anyhow::Context;
use dashmap::DashMap;
use tower_lsp::jsonrpc::{Error as LspError, Result};
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, lsp_types};

use self::config::Config;
use crate::config::CONFIG;
use crate::document::Document;
use crate::fileapi::DEFAULT_QUERY;
use crate::formatting::getformat;
use crate::gammar::{ErrorInformation, LintConfigInfo, checkerror};
use crate::semantic_token::LEGEND_TYPE;
use crate::utils::treehelper::ToPosition;
use crate::utils::{VCPKG_LIBS, VCPKG_PREFIX, did_vcpkg_project, treehelper};
use crate::{
    ast, complete, document_link, fileapi, filewatcher, hover, jump, quick_fix, scansubs,
    semantic_token, utils,
};

static CLIENT_CAPABILITIES: RwLock<Option<TextDocumentClientCapabilities>> = RwLock::new(None);
static ENABLE_SNIPPET: AtomicBool = AtomicBool::new(false);

pub(crate) async fn get_or_update_buffer_contents<P: AsRef<Path>>(
    path: P,
    documents: &DashMap<Uri, Document>,
) -> anyhow::Result<String> {
    let uri = Uri::from_file_path(&path).unwrap();
    if let Some(document) = documents.get(&uri) {
        return Ok(document.text.to_string());
    }
    let text = tokio::fs::read_to_string(&path).await?;
    let document = Document::new(text.clone(), uri.clone()).context("Failed to parse document")?;
    documents.insert(uri, document);
    Ok(text)
}

fn set_client_text_document(text_document: Option<TextDocumentClientCapabilities>) {
    let mut data = CLIENT_CAPABILITIES.write().unwrap();
    *data = text_document;
}

pub fn get_client_capabilities() -> Option<TextDocumentClientCapabilities> {
    let data = CLIENT_CAPABILITIES.read().unwrap();
    data.clone()
}

fn init_snippet_setting(use_snippet: bool) {
    ENABLE_SNIPPET.store(use_snippet, Ordering::Relaxed);
}

pub fn to_use_snippet() -> bool {
    if !ENABLE_SNIPPET.load(Ordering::Relaxed) {
        return false;
    }
    match get_client_capabilities() {
        Some(c) => c
            .completion
            .and_then(|item| item.completion_item)
            .and_then(|item| item.snippet_support)
            .unwrap_or(false),
        _ => false,
    }
}

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

#[derive(Debug)]
pub(crate) struct Backend {
    client: Client,
    documents: DashMap<Uri, Document>,
    /// Storage the message of buffers
    init_info: OnceLock<BackendInitInfo>,
    root_path: OnceLock<Option<PathBuf>>,
}

impl Backend {
    pub(crate) fn new(client: Client) -> Self {
        Self {
            client,
            documents: DashMap::new(),
            init_info: OnceLock::new(),
            root_path: OnceLock::new(),
        }
    }
}

impl Backend {
    fn root_path(&self) -> Option<&PathBuf> {
        self.root_path.get_or_init(|| None).as_ref()
    }

    fn init_info(&self) -> &BackendInitInfo {
        self.init_info
            .get()
            .expect("Should have been inited before")
    }

    fn path_in_project<P: AsRef<Path>>(&self, path: P) -> bool {
        let Some(root_path) = self.root_path() else {
            return true;
        };

        let Some(diff) = pathdiff::diff_paths(path, root_path) else {
            return false;
        };
        diff.components()
            .all(|component| component != Component::ParentDir)
    }

    async fn publish_diagnostics(&self, uri: Uri, context: &str, lint_info: LintConfigInfo) {
        let Ok(file_path) = uri.to_file_path() else {
            tracing::error!("Cannot transport {uri:?} to file_path");
            self.client
                .log_message(
                    MessageType::ERROR,
                    format!("Cannot transport {uri:?} to file_path"),
                )
                .await;
            return;
        };

        if !self.path_in_project(&file_path) {
            return;
        }

        let gammererror = checkerror(&file_path, context, lint_info);
        if let Some(diagnoses) = gammererror {
            let mut pusheddiagnoses = vec![];
            for ErrorInformation {
                start_point,
                end_point,
                message,
                severity,
            } in diagnoses.inner
            {
                let pointx = start_point.to_position();
                let pointy = end_point.to_position();
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
        for item in &self.documents {
            let uri = item.key();
            let document = item.value();
            self.publish_diagnostics(
                uri.clone(),
                &document.text,
                LintConfigInfo {
                    use_lint: self.init_info().enable_lint,
                    use_extra_cmake_lint: true,
                },
            )
            .await;
        }
    }
}

impl LanguageServer for Backend {
    async fn initialize(&self, initial: InitializeParams) -> Result<InitializeResult> {
        let initial_config: Config = initial
            .initialization_options
            .and_then(|value| serde_json::from_value(value).unwrap_or(None))
            .unwrap_or_default();

        init_snippet_setting(initial_config.use_snippets());

        let do_format = initial_config.is_format_enabled();

        let scan_cmake_in_package = initial_config.is_scan_cmake_in_package();

        let enable_lint = initial_config.is_lint_enabled();

        self.init_info
            .set(BackendInitInfo {
                scan_cmake_in_package,
                enable_lint,
            })
            .expect("here should be the first place to init the init_info");

        if let Some(workspace) = initial.capabilities.workspace
            && let Some(watch_file) = workspace.did_change_watched_files
            && let (Some(true), Some(true)) = (
                watch_file.dynamic_registration,
                watch_file.relative_pattern_support,
            )
        {
            // NOTE: I think it only contains one workspace
            if let Some(ref top_path) = initial
                .workspace_folders
                .as_ref()
                .and_then(|folders| folders.first())
                .and_then(|folder| folder.uri.to_file_path().ok())
            {
                let path = top_path.join("build").join("CMakeCache.txt");
                if path.exists() {
                    filewatcher::refresh_error_packages(path);
                }

                tracing::info!("find cache-v2 json, start reading the data");
                let cache_path = top_path
                    .join("build")
                    .join(".cmake")
                    .join("api")
                    .join("v1")
                    .join("reply");
                if cache_path.is_dir() {
                    use std::fs;
                    if let Ok(entries) = fs::read_dir(cache_path) {
                        for entry in entries.flatten() {
                            let file_path = entry.path();
                            if file_path.is_file() {
                                let Some(file_name) = file_path.file_name() else {
                                    continue;
                                };
                                let file_name = file_name.to_string_lossy().to_string();
                                if file_name.starts_with("cache-v2") && file_name.ends_with(".json")
                                {
                                    fileapi::update_cache_data(file_path);
                                    break;
                                }
                            }
                        }
                    }
                }
                tracing::info!("Finish getting the data in cache-v2 json");
            }
        }

        if let Some(ref project_root) = initial
            .workspace_folders
            .as_ref()
            .and_then(|folders| folders.first())
            .and_then(|folder| folder.uri.to_file_path().ok())
        {
            self.root_path
                .set(Some(project_root.to_path_buf()))
                .expect("here should be the only place to set the root_path");
        }

        set_client_text_document(initial.capabilities.text_document);

        let version: String = env!("CARGO_PKG_VERSION").to_string();
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "neocmakelsp".to_string(),
                version: Some(version),
            }),
            capabilities: ServerCapabilities {
                rename_provider: Some(OneOf::Left(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        will_save: Some(false),
                        will_save_wait_until: Some(false),
                        save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                    },
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
                semantic_tokens_provider: if initial_config.enable_semantic_token() {
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

                document_link_provider: Some(DocumentLinkOptions {
                    resolve_provider: Some(true),
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: Some(false),
                    },
                }),
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
                    glob_pattern: GlobPattern::String("**/.cmake/api/v1/reply/*.json".to_string()),
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

        let work_done_token = ProgressToken::Number(1);
        let progress = self
            .client
            .progress(work_done_token, "start initing the workspace")
            .with_message("initial start")
            .with_percentage(0)
            .begin()
            .await;

        if let Some(ref project_root) = self.root_path() {
            progress
                .report_with_message(&format!("start scanning {}", project_root.display()), 10)
                .await;
            scansubs::scan_all(&project_root, true).await;
            let build_dir = project_root.join("build");
            if build_dir.is_dir()
                && let Some(query) = &*DEFAULT_QUERY
            {
                query.write_to_build_dir(build_dir.as_path()).ok();
            }
            if did_vcpkg_project(project_root) {
                progress
                    .report_with_message("find vcpkg dir, start scanning", 20)
                    .await;
                tracing::info!("This project is vcpkg project, start init vcpkg data");
                let vcpkg_installed_path = project_root.join("vcpkg_installed");

                #[cfg(unix)]
                {
                    use crate::utils::packagepkgconfig::QUERYSRULES;
                    // When it is found to be a vcpkg project, the pc will be searched first from the vcpkg download directory.
                    QUERYSRULES.lock().unwrap().insert(
                        0,
                        Box::leak(
                            format!("{}/*.pc", vcpkg_installed_path.to_str().unwrap())
                                .into_boxed_str(),
                        ),
                    );
                }

                // add vcpkg prefix
                VCPKG_PREFIX.lock().unwrap().push(Box::leak(
                    vcpkg_installed_path
                        .to_str()
                        .unwrap()
                        .to_string()
                        .into_boxed_str(),
                ));

                if let Ok(paths) = utils::make_vcpkg_package_search_path(&vcpkg_installed_path) {
                    let mut vcpkg_libs = VCPKG_LIBS.lock().unwrap();
                    for t in paths {
                        vcpkg_libs.push(Box::leak(t.into_boxed_str()));
                    }
                }
            }
        }
        progress
            .report_with_message("Start generating builtin commands", 50)
            .await;
        complete::init_builtin_command();
        progress
            .report_with_message("Start generating builtin module", 55)
            .await;
        complete::init_builtin_module();
        progress
            .report_with_message("Start generating builtin variable", 60)
            .await;
        complete::init_builtin_variable();
        progress
            .report_with_message("Start init system modules", 70)
            .await;
        complete::init_system_modules();
        progress.report_with_message("Scan finished", 100).await;
        progress.finish().await;
    }

    async fn shutdown(&self) -> Result<()> {
        // NOTE: do nothing
        // Seems tower_lsp won't do anything when receive this command.
        // Now it should be proper for me to directly exit(0) here
        exit(0)
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
        let mut has_cached_changed = false;
        for change in params.changes {
            let Ok(file_path) = change.uri.to_file_path() else {
                continue;
            };
            let Some(file_name) = file_path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.to_string())
            else {
                continue;
            };

            if file_name.ends_with("json") && file_name.starts_with("cache-v2") {
                fileapi::update_cache_data(&file_path);
            }
            if file_name.ends_with("txt") {
                has_cached_changed = true;
                if file_name == "CMakeLists.txt" {
                    let Some(path) = self.root_path() else {
                        continue;
                    };
                    scansubs::scan_all(path, false).await;
                    continue;
                }
                self.client
                    .log_message(MessageType::INFO, "CMakeCache changed")
                    .await;
                if let FileChangeType::DELETED = change.typ {
                    filewatcher::clear_error_packages();
                } else {
                    filewatcher::refresh_error_packages(file_path);
                }
            }
        }
        if has_cached_changed {
            self.update_diagnostics().await;
        }
        self.client
            .log_message(MessageType::INFO, "watched files have changed!")
            .await;
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let TextDocumentItem { uri, text, .. } = params.text_document;
        self.update_document(uri.clone(), text.clone()).await;
        let path = match uri.to_file_path() {
            Ok(path) => path,
            Err(_) => {
                tracing::error!("Can't create path from {}", uri.as_str());
                return;
            }
        };

        complete::update_cache(&path, &text).await;
        jump::update_cache(&path, &text).await;
        self.publish_diagnostics(
            uri,
            &text,
            LintConfigInfo {
                use_lint: self.init_info().enable_lint,
                use_extra_cmake_lint: true,
            },
        )
        .await;

        self.client
            .log_message(MessageType::INFO, format!("Opened file {}", path.display()))
            .await;
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let Some(toolong) = params
            .context
            .diagnostics
            .iter()
            .find(|dia| dia.message.starts_with("[C0301]"))
        else {
            return Ok(None);
        };

        let uri = params.text_document.uri;
        let Some(document) = self.documents.get(&uri) else {
            return Ok(None);
        };
        let line = params.range.start.line;
        Ok(quick_fix::lint_fix_action(
            &document.text,
            line,
            toolong,
            uri,
        ))
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.content_changes.into_iter().next().unwrap().text;
        self.update_document(uri.clone(), text.clone()).await;
        if text.lines().count() < 500 {
            self.publish_diagnostics(
                uri.clone(),
                &text,
                LintConfigInfo {
                    use_lint: self.init_info().enable_lint,
                    use_extra_cmake_lint: false,
                },
            )
            .await;
        }
        self.client
            .log_message(MessageType::INFO, &format!("update file: {}", uri.as_str()))
            .await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;

        let has_root = self.root_path().is_some();
        let Some(document) = self.documents.get(&uri) else {
            self.client
                .log_message(MessageType::INFO, "file saved!")
                .await;
            return;
        };
        let file_path = match uri.to_file_path() {
            Ok(file_path) => file_path,
            Err(_) => {
                tracing::error!("Cannot get file_path from {}", uri.as_str());
                return;
            }
        };
        if has_root {
            scansubs::scan_dir(&file_path, false).await;
            complete::update_cache(&file_path, &document.text).await;
            jump::update_cache(&file_path, &document.text).await;
        }
        self.publish_diagnostics(
            uri,
            &document.text,
            LintConfigInfo {
                use_lint: self.init_info().enable_lint,
                use_extra_cmake_lint: CONFIG.enable_external_cmake_lint,
            },
        )
        .await;

        self.client
            .log_message(MessageType::INFO, "file saved!")
            .await;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let position = params.text_document_position_params.position;
        let uri = params.text_document_position_params.text_document.uri;
        let Some(document) = self.documents.get(&uri) else {
            return Ok(None);
        };
        let output =
            hover::get_hovered_doc(position, document.tree.root_node(), &document.text).await;
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
    }

    async fn formatting(&self, input: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        self.client
            .log_message(
                MessageType::INFO,
                format!("formatting, space is {}", input.options.insert_spaces),
            )
            .await;
        let uri = input.text_document.uri;
        let space_line = if input.options.insert_spaces {
            input.options.tab_size
        } else {
            1
        };
        let insert_final_newline = input.options.insert_final_newline.unwrap_or(false);

        let Some(document) = self.documents.get(&uri) else {
            return Ok(None);
        };
        Ok(getformat(
            self.root_path().map(|p| p.as_path()),
            &document.text,
            &self.client,
            space_line,
            input.options.insert_spaces,
            insert_final_newline,
        )
        .await)
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.client
            .log_message(
                MessageType::INFO,
                format!("file {:?} closed!", params.text_document.uri),
            )
            .await;
    }

    async fn completion(&self, input: CompletionParams) -> Result<Option<CompletionResponse>> {
        self.client.log_message(MessageType::INFO, "Complete").await;
        let location = input.text_document_position.position;
        let uri = input.text_document_position.text_document.uri;
        let file_path = match uri.to_file_path() {
            Ok(file_path) => file_path,
            Err(_) => {
                tracing::error!("Cannot get file_path from {}", uri.as_str());
                return Err(LspError::internal_error());
            }
        };
        let Some(document) = self.documents.get(&uri) else {
            return Ok(None);
        };
        Ok(complete::getcomplete(
            &document.text,
            location,
            &self.client,
            &file_path,
            self.init_info().scan_cmake_in_package,
            &self.documents,
        )
        .await)
    }

    async fn references(&self, input: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = input.text_document_position.text_document.uri;
        let location = input.text_document_position.position;
        let Some(document) = self.documents.get(&uri) else {
            return Ok(None);
        };
        let file_path = match uri.to_file_path() {
            Ok(file_path) => file_path,
            Err(_) => {
                tracing::error!("Cannot get file_path from {uri:?}");
                return Err(LspError::internal_error());
            }
        };
        Ok(jump::godef(
            location,
            &document.text,
            &file_path,
            &self.client,
            false,
            false,
            &self.documents,
        )
        .await)
    }

    async fn rename(&self, input: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let edited = input.new_name;
        let uri = input.text_document_position.text_document.uri;
        let location = input.text_document_position.position;
        let Some(document) = self.documents.get(&uri) else {
            return Ok(None);
        };
        let file_path = match uri.to_file_path() {
            Ok(file_path) => file_path,
            Err(_) => {
                tracing::error!("Cannot get file_path from {uri:?}");
                return Err(LspError::internal_error());
            }
        };
        Ok(self
            .rename(&edited, location, file_path, &document.text)
            .await)
    }

    async fn goto_definition(
        &self,
        input: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = input.text_document_position_params.text_document.uri;
        let location = input.text_document_position_params.position;
        let Some(document) = self.documents.get(&uri) else {
            return Ok(None);
        };

        let origin_selection_range =
            treehelper::get_position_range(location, document.tree.root_node());

        let file_path = match uri.to_file_path() {
            Ok(file_path) => file_path,
            Err(_) => {
                tracing::error!("Cannot get file_path from {uri:?}");
                return Err(LspError::internal_error());
            }
        };
        match jump::godef(
            location,
            &document.text,
            &file_path,
            &self.client,
            true,
            false,
            &self.documents,
        )
        .await
        {
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
    }

    async fn document_symbol(
        &self,
        input: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = input.text_document.uri;
        let Some(document) = self.documents.get(&uri) else {
            return Ok(None);
        };
        Ok(ast::getast(&self.client, &document.text).await)
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri.clone();
        let Some(document) = self.documents.get(&uri) else {
            return Ok(None);
        };
        Ok(semantic_token::semantic_token(&self.client, &document.text).await)
    }

    async fn document_link(&self, input: DocumentLinkParams) -> Result<Option<Vec<DocumentLink>>> {
        let uri = input.text_document.uri;
        let file_path = match uri.to_file_path() {
            Ok(file_path) => file_path,
            Err(_) => {
                tracing::error!("Cannot get file_path from {uri:?}");
                return Err(LspError::internal_error());
            }
        };
        let Some(document) = self.documents.get(&uri) else {
            return Ok(None);
        };
        Ok(document_link::document_link_search(
            &document.text,
            file_path,
        ))
    }
}

impl Backend {
    async fn update_document(&self, uri: Uri, text: String) {
        let Some(document) = Document::new(text, uri.clone()) else {
            self.client
                .log_message(
                    MessageType::ERROR,
                    format!("Failed to parse document {}", uri.as_str()),
                )
                .await;
            return;
        };
        self.documents.insert(uri, document);
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;

    use serde::Serialize;
    use tempfile::tempdir;
    use tower::Service;
    use tower::util::ServiceExt;
    use tower_lsp::jsonrpc::Request;
    use tower_lsp::lsp_types::{
        CompletionParams, CompletionResponse, DidOpenTextDocumentParams, InitializeParams,
        InitializeResult, PartialResultParams, Position, SemanticTokensFullOptions,
        SemanticTokensLegend, SemanticTokensOptions, SemanticTokensServerCapabilities,
        TextDocumentIdentifier, TextDocumentItem, TextDocumentPositionParams, Uri,
        WorkDoneProgressOptions, WorkDoneProgressParams, WorkspaceFolder,
    };
    use tower_lsp::{LanguageServer, LspService};

    use super::*;
    use crate::languageserver::Config;
    use crate::semantic_token::LEGEND_TYPE;

    fn create_request<T>(id: i64, init_param: T, method: &'static str) -> Request
    where
        T: Serialize,
    {
        Request::build(method)
            .params(serde_json::to_value(&init_param).unwrap())
            .id(id)
            .finish()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_init() {
        let file_info =
            "set(AB \"100\")\r\n# test hello \r\nfunction(bb)\r\nendfunction()\r\nset(FF ${A} )";
        let dir = tempdir().unwrap();
        let root_cmake = dir.path().join("CMakeList.txt");
        let mut file = File::create(&root_cmake).unwrap();
        writeln!(file, "{}", &file_info).unwrap();

        let (mut service, _) = LspService::new(Backend::new);

        #[cfg(unix)]
        let init_param = InitializeParams {
            workspace_folders: Some(vec![WorkspaceFolder {
                name: "main".to_string(),
                uri: Uri::from_file_path("/tmp").unwrap(),
            }]),
            initialization_options: Some(
                serde_json::to_value(Config {
                    semantic_token: Some(true),
                    ..Default::default()
                })
                .unwrap(),
            ),
            ..Default::default()
        };
        #[cfg(not(unix))]
        let init_param = InitializeParams {
            workspace_folders: Some(vec![WorkspaceFolder {
                name: "main".to_string(),
                uri: Uri::from_file_path(r"C:\\Windows\\System").unwrap(),
            }]),
            initialization_options: Some(
                serde_json::to_value(Config {
                    semantic_token: Some(true),
                    ..Default::default()
                })
                .unwrap(),
            ),
            ..Default::default()
        };

        let request = create_request(1, init_param, "initialize");
        let response = service.ready().await.unwrap().call(request).await.unwrap();

        let init_result: InitializeResult =
            serde_json::from_value(response.unwrap().result().unwrap().clone()).unwrap();

        assert_eq!(
            init_result.capabilities.semantic_tokens_provider,
            Some(SemanticTokensServerCapabilities::SemanticTokensOptions(
                SemanticTokensOptions {
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None
                    },
                    legend: SemanticTokensLegend {
                        token_types: LEGEND_TYPE.into(),
                        token_modifiers: [].to_vec()
                    },
                    range: None,
                    full: Some(SemanticTokensFullOptions::Bool(true)),
                }
            ))
        );
        let backend = service.inner();
        #[cfg(unix)]
        {
            assert!(backend.path_in_project(Path::new("/tmp/helloworld/")));
            assert!(!backend.path_in_project(Path::new("/home/helloworld/")));
        }
        #[cfg(not(unix))]
        {
            assert!(backend.path_in_project(Path::new(r"C:\\Windows\\System\\FolderA")));
            assert!(!backend.path_in_project(Path::new(r"C:\\Windows")));
        }

        let test_url = Uri::from_file_path(root_cmake.clone()).unwrap();
        let open_params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: test_url.clone(),
                text: file_info.to_string(),
                version: 0,
                language_id: "cmake".to_string(),
            },
        };
        backend.did_open(open_params).await;

        let complete_param = CompletionParams {
            text_document_position: TextDocumentPositionParams {
                position: Position {
                    line: 4,
                    character: 10,
                },
                text_document: TextDocumentIdentifier { uri: test_url },
            },
            context: None,
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        let request = create_request(3, complete_param, "textDocument/completion");
        let response = service.ready().await.unwrap().call(request).await.unwrap();

        let _complete_result: CompletionResponse =
            serde_json::from_value(response.unwrap().result().unwrap().clone()).unwrap();
        println!("{:?}", _complete_result);
    }
}
