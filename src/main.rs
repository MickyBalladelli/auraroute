use auraroute::app::{build_app, load_tokenizer, AppState};
use auraroute::config::AppConfig;
use reqwest::Client;
use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "auraroute=info".into()),
        )
        .init();

    let client = Client::new();
    let config = AppConfig::load()?;
    info!(models = config.models.len(), "configured local model(s)");
    for model in &config.models {
        info!(name = %model.name, upstream = %model.upstream, "model route");
    }
    let tokenizer = load_tokenizer(&config)?;
    if let Some(path) = config.tokenizer_path.as_deref() {
        info!(tokenizer = path, "tokenizer loaded");
    } else {
        info!("tokenizer: whitespace fallback");
    }
    let listen = config.listen.clone();

    let app = build_app(AppState {
        client,
        config,
        tokenizer,
    });

    let listener = TcpListener::bind(&listen).await?;
    let addr = listener.local_addr()?;
    info!(%addr, "listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("shutdown complete");
    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
    info!("received SIGINT, starting graceful shutdown…");
}