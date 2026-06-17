use std::net::SocketAddr;

use axum::extract::{Json, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::Router;
use auraroute::proxy::proxy_stream_to_client;
use reqwest::Client;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

#[derive(Clone)]
struct ProxyState {
    client: Client,
    upstream_url: String,
}

async fn mock_local_sse(Json(_payload): Json<Value>) -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
        "data: local-one\n\ndata: local-two\n\n",
    )
}

async fn proxy_handler(
    State(state): State<ProxyState>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    match proxy_stream_to_client(&state.client, &state.upstream_url, payload).await {
        Ok((stream, _upstream_headers)) => stream.into_response(),
        Err(error) => (StatusCode::BAD_GATEWAY, error).into_response(),
    }
}

async fn spawn_mock_server(app: Router) -> Result<(SocketAddr, JoinHandle<()>), std::io::Error> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let handle = tokio::spawn(async move {
        let result = axum::serve(listener, app).await;
        if let Err(error) = result {
            eprintln!("test server stopped with error: {error}");
        }
    });

    Ok((addr, handle))
}

#[tokio::test]
async fn proxy_streams_mock_local_sse_chunks_to_client() -> Result<(), Box<dyn std::error::Error>> {
    let upstream_app = Router::new().route("/v1/chat/completions", post(mock_local_sse));
    let (upstream_addr, upstream_handle) = spawn_mock_server(upstream_app).await?;
    let upstream_url = format!("http://{upstream_addr}/v1/chat/completions");

    let proxy_state = ProxyState {
        client: Client::new(),
        upstream_url,
    };
    let proxy_app = Router::new()
        .route("/proxy", post(proxy_handler))
        .with_state(proxy_state);
    let (proxy_addr, proxy_handle) = spawn_mock_server(proxy_app).await?;

    let response = Client::new()
        .post(format!("http://{proxy_addr}/proxy"))
        .json(&json!({
            "model": "auraroute-test",
            "stream": true,
            "messages": [
                {
                    "role": "user",
                    "content": "hello local model"
                }
            ]
        }))
        .send()
        .await?;

    assert!(response.status().is_success());

    let body = response.text().await?;
    assert!(body.contains("local-one"));
    assert!(body.contains("local-two"));

    proxy_handle.abort();
    upstream_handle.abort();

    Ok(())
}