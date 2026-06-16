use axum::extract::{Json, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::Router;
use auraroute::config::AppConfig;
use auraroute::{hardware, models, proxy, scorer};
use reqwest::Client;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

#[derive(Clone)]
pub struct AppState {
    client: Client,
    config: AppConfig,
}

pub async fn handle_chat_completion(
    State(state): State<AppState>,
    Json(payload): Json<models::ChatCompletionRequest>,
) -> impl IntoResponse {
    let prompt = models::extract_user_prompt(&payload);
    let token_count = prompt.split_whitespace().count();
    let complexity_score = scorer::calculate_complexity(&prompt, token_count);
    let resources = hardware::get_current_resources();
    let local_pressure = hardware::has_local_resource_pressure(complexity_score, &resources);
    let Some(model) = state.config.select_model(complexity_score) else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "no local model routes configured".to_string(),
        )
            .into_response();
    };

    println!(
        "[AuraRoute] Score: {}, VRAM: {} MB, CPU: {:.1}%, pressure: {} -> Routing to {}",
        complexity_score, resources.free_vram_mb, resources.cpu_usage_pct, local_pressure, model.name
    );

    let json_value = match serde_json::to_value(&payload) {
        Ok(value) => value,
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to serialize chat completion payload: {error}"),
            )
                .into_response();
        }
    };

    match proxy::proxy_stream_to_client(&state.client, &model.upstream, json_value).await {
        Ok(stream) => stream.into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("upstream proxy error: {error}"),
        )
            .into_response(),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = Client::new();
    let config = AppConfig::load()?;
    println!("[AuraRoute] Configured {} local model(s)", config.models.len());
    for model in &config.models {
        println!("[AuraRoute] Model '{}': {}", model.name, model.upstream);
    }
    let listen = config.listen.clone();

    let app = Router::new()
        .route("/v1/chat/completions", post(handle_chat_completion))
        .with_state(AppState { client, config })
        .layer(CorsLayer::permissive());

    let listener = TcpListener::bind(&listen).await?;
    println!("[AuraRoute] Listening on http://{listen}");

    axum::serve(listener, app).await?;

    Ok(())
}
