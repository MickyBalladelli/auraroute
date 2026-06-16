use auraroute::app::{build_app, AppState};
use auraroute::config::AppConfig;
use reqwest::Client;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = Client::new();
    let config = AppConfig::load()?;
    println!("[AuraRoute] Configured {} local model(s)", config.models.len());
    for model in &config.models {
        println!("[AuraRoute] Model '{}': {}", model.name, model.upstream);
    }
    let listen = config.listen.clone();

    let app = build_app(AppState { client, config });

    let listener = TcpListener::bind(&listen).await?;
    println!("[AuraRoute] Listening on http://{listen}");

    axum::serve(listener, app).await?;

    Ok(())
}
