use std::sync::Arc;

use tokio::sync::Mutex;
use tower::util::ServiceExt;
use tower::Service;
use tower_lsp::jsonrpc::Request;
use tower_lsp::lsp_types::{
    InitializeParams, InitializeResult, SemanticTokensFullOptions, SemanticTokensLegend,
    SemanticTokensOptions, SemanticTokensServerCapabilities, Url, WorkDoneProgressOptions,
    WorkspaceFolder,
};
use tower_lsp::LspService;

use super::Backend;
use crate::languageserver::Config;
use crate::semantic_token::LEGEND_TYPE;
use crate::BackendInitInfo;

fn initialize_request(id: i64, init_param: InitializeParams) -> Request {
    Request::build("initialize")
        .params(serde_json::to_value(&init_param).unwrap())
        .id(id)
        .finish()
}

#[tokio::test(flavor = "current_thread")]
async fn test_init() {
    let (mut service, _) = LspService::new(|client| Backend {
        client,
        init_info: Arc::new(Mutex::new(BackendInitInfo {
            scan_cmake_in_package: true,
            enable_lint: true,
        })),
        root_path: Arc::new(Mutex::new(None)),
    });

    #[cfg(unix)]
    let init_param = InitializeParams {
        workspace_folders: Some(vec![WorkspaceFolder {
            name: "main".to_string(),
            uri: Url::from_file_path("/tmp").unwrap(),
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
            uri: Url::from_file_path(r"C:\\Windows\\System").unwrap(),
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

    let request = initialize_request(1, init_param.clone());
    let response = service
        .ready()
        .await
        .unwrap()
        .call(request.clone())
        .await
        .unwrap();

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
}
