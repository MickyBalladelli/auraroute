use std::time::Duration;

use axum::extract::{Json, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use reqwest::Client;
use serde::Serialize;
use tower_http::cors::CorsLayer;

use crate::config::AppConfig;
use crate::{hardware, models, proxy, scorer};

#[derive(Clone)]
pub struct AppState {
    pub client: Client,
    pub config: AppConfig,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    listen: String,
    resources: ResourceSnapshot,
    models: Vec<ModelHealth>,
}

#[derive(Debug, Serialize)]
struct ResourceSnapshot {
    free_vram_mb: u64,
    cpu_usage_pct: f32,
}

#[derive(Debug, Serialize)]
struct ModelHealth {
    name: String,
    upstream: String,
    min_complexity: Option<u8>,
    max_complexity: Option<u8>,
    kind: Option<crate::config::ModelKind>,
    reachable: bool,
    status_code: Option<u16>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct ModelsResponse {
    object: &'static str,
    data: Vec<ModelDescriptor>,
}

#[derive(Debug, Serialize)]
struct ModelDescriptor {
    id: String,
    object: &'static str,
    owned_by: &'static str,
    upstream: String,
    min_complexity: Option<u8>,
    max_complexity: Option<u8>,
    kind: Option<crate::config::ModelKind>,
    reachable: bool,
    status_code: Option<u16>,
}

pub fn build_app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(handle_health))
        .route("/v1/models", get(handle_models))
        .route("/v1/chat/completions", post(handle_chat_completion))
        .with_state(state)
        .layer(CorsLayer::permissive())
}

pub async fn handle_chat_completion(
    State(state): State<AppState>,
    Json(payload): Json<models::ChatCompletionRequest>,
) -> impl IntoResponse {
    let prompt = models::extract_user_prompt(&payload);
    let token_count = prompt.split_whitespace().count();
    let complexity_score = scorer::calculate_complexity(&prompt, token_count);
    let code_prompt = scorer::looks_like_code(&prompt);
    let resources = hardware::get_current_resources();
    let local_pressure = hardware::has_local_resource_pressure(complexity_score, &resources);
    let Some(model) = state.config.select_model(complexity_score, code_prompt) else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "no local model routes configured".to_string(),
        )
            .into_response();
    };

    println!(
        "[AuraRoute] Score: {}, code: {}, VRAM: {} MB, CPU: {:.1}%, pressure: {} -> Routing to {}",
        complexity_score,
        code_prompt,
        resources.free_vram_mb,
        resources.cpu_usage_pct,
        local_pressure,
        model.name
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

pub async fn handle_health(State(state): State<AppState>) -> impl IntoResponse {
    let resources = hardware::get_current_resources();
    let models = probe_models(&state).await;
    let all_reachable = models.iter().all(|model| model.reachable);
    let status = if all_reachable { "ok" } else { "degraded" };
    let response = HealthResponse {
        status,
        listen: state.config.listen.clone(),
        resources: ResourceSnapshot {
            free_vram_mb: resources.free_vram_mb,
            cpu_usage_pct: resources.cpu_usage_pct,
        },
        models,
    };

    let code = if all_reachable {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (code, Json(response))
}

pub async fn handle_models(State(state): State<AppState>) -> impl IntoResponse {
    let health = probe_models(&state).await;
    let data = health
        .into_iter()
        .map(|model| ModelDescriptor {
            id: model.name,
            object: "model",
            owned_by: "local",
            upstream: model.upstream,
            min_complexity: model.min_complexity,
            max_complexity: model.max_complexity,
            kind: model.kind,
            reachable: model.reachable,
            status_code: model.status_code,
        })
        .collect();

    Json(ModelsResponse {
        object: "list",
        data,
    })
}

async fn probe_models(state: &AppState) -> Vec<ModelHealth> {
    let mut models = Vec::with_capacity(state.config.models.len());

    for model in &state.config.models {
        let probe = probe_upstream(&state.client, &model.upstream).await;
        models.push(ModelHealth {
            name: model.name.clone(),
            upstream: model.upstream.clone(),
            min_complexity: model.min_complexity,
            max_complexity: model.max_complexity,
            kind: model.kind,
            reachable: probe.reachable,
            status_code: probe.status_code,
            error: probe.error,
        });
    }

    models
}

struct ProbeResult {
    reachable: bool,
    status_code: Option<u16>,
    error: Option<String>,
}

async fn probe_upstream(client: &Client, upstream: &str) -> ProbeResult {
    let result = client
        .get(upstream)
        .timeout(Duration::from_secs(2))
        .send()
        .await;

    match result {
        Ok(response) => {
            let status = response.status();
            ProbeResult {
                reachable: true,
                status_code: Some(status.as_u16()),
                error: None,
            }
        }
        Err(error) => ProbeResult {
            reachable: false,
            status_code: error.status().map(|status| status.as_u16()),
            error: Some(error.to_string()),
        },
    }
}
