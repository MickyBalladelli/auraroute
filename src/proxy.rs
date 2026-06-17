use std::convert::Infallible;
use std::time::Duration;

use axum::response::sse::{Event, Sse};
use futures_util::stream::{Stream, StreamExt};
use reqwest::Client;
use reqwest::header::HeaderMap;

const DEFAULT_UPSTREAM_TIMEOUT_SECS: u64 = 300;

pub async fn proxy_stream_to_client(
    client: &Client,
    upstream_url: &str,
    payload: serde_json::Value,
) -> Result<(Sse<impl Stream<Item = Result<Event, Infallible>>>, HeaderMap), String> {
    let response = client
        .post(upstream_url)
        .json(&payload)
        .timeout(Duration::from_secs(DEFAULT_UPSTREAM_TIMEOUT_SECS))
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                format!("upstream request timed out after {DEFAULT_UPSTREAM_TIMEOUT_SECS}s")
            } else {
                e.to_string()
            }
        })?;

    let status = response.status();
    let upstream_headers = response.headers().clone();

    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "(could not read upstream error body)".to_string());
        return Err(format!(
            "upstream returned {}: {}",
            status.as_u16(),
            body
        ));
    }

    let mapped_stream = response.bytes_stream().map(|chunk| match chunk {
        Ok(bytes) => {
            let text_string = String::from_utf8_lossy(&bytes).to_string();
            Ok(Event::default().data(text_string))
        }
        Err(_) => Ok(Event::default().data("[ERROR: AuraRoute stream interrupted]")),
    });

    Ok((Sse::new(mapped_stream), upstream_headers))
}