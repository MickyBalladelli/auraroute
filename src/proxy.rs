use std::convert::Infallible;

use axum::response::sse::{Event, Sse};
use futures_util::stream::{Stream, StreamExt};
use reqwest::Client;

pub async fn proxy_stream_to_client(
    client: &Client,
    upstream_url: &str,
    payload: serde_json::Value,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, String> {
    let response = client
        .post(upstream_url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let mapped_stream = response.bytes_stream().map(|chunk| match chunk {
        Ok(bytes) => {
            let text_string = String::from_utf8_lossy(&bytes).to_string();
            Ok(Event::default().data(text_string))
        }
        Err(_) => Ok(Event::default().data("[ERROR: AuraRoute stream interrupted]")),
    });

    Ok(Sse::new(mapped_stream))
}
