use std::sync::Arc;

use tokio::sync::Mutex;
use tower::{util::ServiceExt, Service};
use tower_lsp::{
    jsonrpc::{Request, Response},
    lsp_types::{InitializeParams, Url},
    LspService,
};

use crate::BackendInitInfo;

use super::Backend;

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
        root_uri: Some(Url::from_file_path("/tmp").unwrap()),
        ..Default::default()
    };
    #[cfg(not(unix))]
    let init_param = InitializeParams {
        root_uri: Some(Url::from_file_path(r"C:\\Windows\\System").unwrap()),
        ..Default::default()
    };

    let request = initialize_request(1, init_param.clone());
    let _response = service.ready().await.unwrap().call(request.clone()).await;
    let _ = Response::from_ok(1.into(), serde_json::to_value(&init_param).unwrap());
}
