use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Json, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use reqwest::Client;
use serde::Serialize;
use tokenizers::Tokenizer;
use tower_http::cors::CorsLayer;
use tracing::{debug, error, info, warn};

use crate::config::AppConfig;
use crate::{hardware, models, proxy, scorer};

#[derive(Clone)]
pub struct AppState {
    pub client: Client,
    pub config: AppConfig,
    pub tokenizer: Option<Arc<Tokenizer>>,
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
    let token_count = count_tokens(&prompt, state.tokenizer.as_deref());
    let complexity_score = scorer::calculate_complexity(&prompt, token_count);
    let code_prompt = scorer::looks_like_code(&prompt);
    let resources = hardware::get_current_resources();
    let local_pressure = hardware::has_local_resource_pressure(complexity_score, &resources);
    let Some(model) = state.config.select_model(complexity_score, code_prompt) else {
        error!("No local model routes configured");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "no local model routes configured".to_string(),
        )
            .into_response();
    };

    info!(
        score = complexity_score,
        code = code_prompt,
        vram_mb = resources.free_vram_mb,
        cpu_pct = resources.cpu_usage_pct,
        pressure = local_pressure,
        model = %model.name,
        "routing request"
    );

    let json_value = match serde_json::to_value(&payload) {
        Ok(value) => value,
        Err(error) => {
            error!(%error, "failed to serialize chat completion payload");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to serialize chat completion payload: {error}"),
            )
                .into_response();
        }
    };

    match proxy::proxy_stream_to_client(&state.client, &model.upstream, json_value).await {
        Ok((stream, upstream_headers)) => {
            let mut response = stream.into_response();
            let response_headers = response.headers_mut();
            for (key, value) in upstream_headers.iter() {
                let key_lower = key.as_str().to_ascii_lowercase();
                // Skip hop-by-hop headers since SSE uses chunked encoding
                if !matches!(
                    key_lower.as_str(),
                    "transfer-encoding" | "connection" | "keep-alive"
                ) {
                    response_headers.insert(key.clone(), value.clone());
                }
            }
            response
        }
        Err(error) => {
            error!(%error, "upstream proxy error");
            (StatusCode::BAD_GATEWAY, error).into_response()
        }
    }
}

pub fn load_tokenizer(config: &AppConfig) -> Result<Option<Arc<Tokenizer>>, String> {
    let Some(path) = config.tokenizer_path.as_deref() else {
        return Ok(None);
    };

    Tokenizer::from_file(path)
        .map(Arc::new)
        .map(Some)
        .map_err(|error| format!("failed to load tokenizer '{path}': {error}"))
}

fn count_tokens(prompt: &str, tokenizer: Option<&Tokenizer>) -> usize {
    if let Some(tokenizer) = tokenizer {
        match tokenizer.encode(prompt, true) {
            Ok(encoding) => return encoding.len(),
            Err(error) => {
                warn!(%error, "tokenizer failed, falling back to whitespace token count");
            }
        }
    }

    prompt.split_whitespace().count()
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
        let probe = probe_upstream(&state.client, model.health_url()).await;
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

/// Probes an upstream URL by sending an OPTIONS request.
/// Falls back to a GET if OPTIONS is not supported (405).
/// If the user configured a specific `health_url` per model, that endpoint
/// is probed directly.
async fn probe_upstream(client: &Client, url: &str) -> ProbeResult {
    match try_probe(client, url).await {
        Ok(status) => ProbeResult {
            reachable: true,
            status_code: Some(status.as_u16()),
            error: None,
        },
        Err(error) => ProbeResult {
            reachable: false,
            status_code: error.status().map(|s| s.as_u16()),
            error: Some(error.to_string()),
        },
    }
}

async fn try_probe(client: &Client, url: &str) -> reqwest::Result<reqwest::StatusCode> {
    debug!(%url, "probing upstream");

    // Try OPTIONS first — most HTTP servers respond without side effects
    let response = client
        .request(reqwest::Method::OPTIONS, url)
        .timeout(Duration::from_secs(2))
        .send()
        .await?;

    if response.status() != reqwest::StatusCode::METHOD_NOT_ALLOWED {
        debug!(%url, status = %response.status().as_u16(), "upstream reachable via OPTIONS");
        return Ok(response.status());
    }

    // Fall back to GET for servers that don't support OPTIONS
    let response = client
        .get(url)
        .timeout(Duration::from_secs(2))
        .send()
        .await?;

    debug!(%url, status = %response.status().as_u16(), "upstream reachable via GET");
    Ok(response.status())
}

/// Returns the local address the router is bound to.
/// Useful for startup messages and shutdown notifications.
pub fn bound_address(listen: &str) -> Option<SocketAddr> {
    listen.parse().ok()
}