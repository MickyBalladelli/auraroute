use std::net::SocketAddr;

use axum::routing::get;
use axum::Router;
use auraroute::app::{build_app, AppState};
use auraroute::config::{AppConfig, ModelKind, ModelRoute};
use reqwest::Client;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

async fn ok_probe() -> &'static str {
    "ok"
}

async fn mock_chat() -> &'static str {
    "data: ok\n\n"
}

async fn spawn_server(app: Router) -> Result<(SocketAddr, JoinHandle<()>), std::io::Error> {
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
async fn health_and_models_report_reachable_local_routes() -> Result<(), Box<dyn std::error::Error>> {
    let upstream_app = Router::new()
        .route("/v1/chat/completions", get(ok_probe).post(mock_chat));
    let (upstream_addr, upstream_handle) = spawn_server(upstream_app).await?;

    let config = AppConfig {
        listen: "127.0.0.1:0".to_string(),
        models: vec![ModelRoute {
            name: "fast".to_string(),
            upstream: format!("http://{upstream_addr}/v1/chat/completions"),
            kind: Some(ModelKind::Fast),
            min_complexity: None,
            max_complexity: Some(2),
        }],
    };

    let app = build_app(AppState {
        client: Client::new(),
        config,
    });
    let (app_addr, app_handle) = spawn_server(app).await?;

    let health = Client::new()
        .get(format!("http://{app_addr}/health"))
        .send()
        .await?;
    assert!(health.status().is_success());
    let health_json: serde_json::Value = health.json().await?;
    assert_eq!(health_json["status"], "ok");
    assert_eq!(health_json["models"][0]["name"], "fast");
    assert_eq!(health_json["models"][0]["reachable"], true);

    let models = Client::new()
        .get(format!("http://{app_addr}/v1/models"))
        .send()
        .await?;
    assert!(models.status().is_success());
    let models_json: serde_json::Value = models.json().await?;
    assert_eq!(models_json["object"], "list");
    assert_eq!(models_json["data"][0]["id"], "fast");
    assert_eq!(models_json["data"][0]["reachable"], true);

    app_handle.abort();
    upstream_handle.abort();

    Ok(())
}
