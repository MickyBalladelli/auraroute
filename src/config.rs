use std::env;

const DEFAULT_LOCAL_UPSTREAM: &str = "http://localhost:11434/v1/chat/completions";

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub local_upstream: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            local_upstream: env::var("AURAROUTE_LOCAL_UPSTREAM")
                .unwrap_or_else(|_| DEFAULT_LOCAL_UPSTREAM.to_string()),
        }
    }
}
