use std::path::Path;

use serde::Serialize;
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

use super::Backend;
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
    use std::fs::File;
    use std::io::Write;

    use tempfile::tempdir;

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
