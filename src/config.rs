use std::env;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

const DEFAULT_CONFIG_PATH: &str = "auraroute.toml";
const DEFAULT_LISTEN: &str = "127.0.0.1:8080";
const DEFAULT_FAST_UPSTREAM: &str = "http://localhost:11434/v1/chat/completions";
const DEFAULT_CODE_UPSTREAM: &str = "http://localhost:11435/v1/chat/completions";
const DEFAULT_REASONING_UPSTREAM: &str = "http://localhost:11436/v1/chat/completions";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AppConfig {
    #[serde(default = "default_listen")]
    pub listen: String,
    #[serde(default)]
    pub tokenizer_path: Option<String>,
    #[serde(default = "default_models")]
    pub models: Vec<ModelRoute>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelRoute {
    pub name: String,
    pub upstream: String,
    #[serde(default)]
    pub kind: Option<ModelKind>,
    pub min_complexity: Option<u8>,
    pub max_complexity: Option<u8>,
    #[serde(default)]
    pub health_url: Option<String>,
}

impl AppConfig {
    pub fn load() -> Result<Self, ConfigError> {
        let path = env::var("AURAROUTE_CONFIG").unwrap_or_else(|_| DEFAULT_CONFIG_PATH.to_string());

        if Path::new(&path).exists() {
            let raw = fs::read_to_string(&path).map_err(|source| ConfigError::Read {
                path: path.clone(),
                source,
            })?;
            let mut config = toml::from_str::<Self>(&raw).map_err(|source| ConfigError::Parse {
                path: path.clone(),
                source,
            })?;
            config.apply_env_overrides();
            config.validate()?;
            return Ok(config);
        }

        let mut config = Self {
            listen: default_listen(),
            tokenizer_path: None,
            models: default_models(),
        };
        config.apply_env_overrides();
        config.validate()?;
        Ok(config)
    }

    pub fn select_model(&self, complexity_score: u8, code_prompt: bool) -> Option<&ModelRoute> {
        if code_prompt {
            if let Some(model) = self.find_preferred(complexity_score, ModelKind::Code) {
                return Some(model);
            }
        }

        if complexity_score >= 3 {
            if let Some(model) = self.find_preferred(complexity_score, ModelKind::Reasoning) {
                return Some(model);
            }
        }

        if complexity_score <= 2 {
            if let Some(model) = self.find_preferred(complexity_score, ModelKind::Fast) {
                return Some(model);
            }
        }

        self.models
            .iter()
            .find(|model| model.matches_complexity(complexity_score))
            .or_else(|| self.models.first())
    }

    fn apply_env_overrides(&mut self) {
        if let Ok(listen) = env::var("AURAROUTE_LISTEN") {
            self.listen = listen;
        }

        if let Ok(upstream) = env::var("AURAROUTE_LOCAL_UPSTREAM") {
            if let Some(model) = self.models.first_mut() {
                model.upstream = upstream;
            }
        }

        if let Ok(tokenizer_path) = env::var("AURAROUTE_TOKENIZER_PATH") {
            self.tokenizer_path = Some(tokenizer_path);
        }
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.listen.trim().is_empty() {
            return Err(ConfigError::Invalid(
                "listen address cannot be empty".to_string(),
            ));
        }

        if self.models.is_empty() {
            return Err(ConfigError::Invalid(
                "at least one local model route is required".to_string(),
            ));
        }

        for model in &self.models {
            model.validate()?;
        }

        Ok(())
    }

    fn find_preferred(&self, complexity_score: u8, kind: ModelKind) -> Option<&ModelRoute> {
        self.models
            .iter()
            .find(|model| model.matches_kind(kind) && model.matches_complexity(complexity_score))
            .or_else(|| self.models.iter().find(|model| model.matches_kind(kind)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelKind {
    Fast,
    Code,
    Reasoning,
}

impl ModelRoute {
    pub fn matches_complexity(&self, complexity_score: u8) -> bool {
        let above_min = self
            .min_complexity
            .map(|minimum| complexity_score >= minimum)
            .unwrap_or(true);
        let below_max = self
            .max_complexity
            .map(|maximum| complexity_score <= maximum)
            .unwrap_or(true);

        above_min && below_max
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.name.trim().is_empty() {
            return Err(ConfigError::Invalid("model name cannot be empty".to_string()));
        }

        if self.upstream.trim().is_empty() {
            return Err(ConfigError::Invalid(format!(
                "model '{}' upstream cannot be empty",
                self.name
            )));
        }

        if let (Some(minimum), Some(maximum)) = (self.min_complexity, self.max_complexity) {
            if minimum > maximum {
                return Err(ConfigError::Invalid(format!(
                    "model '{}' min_complexity cannot exceed max_complexity",
                    self.name
                )));
            }
        }

        if self.kind.is_none() {
            return Err(ConfigError::Invalid(format!(
                "model '{}' must have an explicit 'kind' (fast, code, or reasoning)",
                self.name
            )));
        }

        Ok(())
    }

    fn matches_kind(&self, kind: ModelKind) -> bool {
        self.kind == Some(kind)
    }

    pub fn health_url(&self) -> &str {
        self.health_url.as_deref().unwrap_or(&self.upstream)
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config '{path}': {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse config '{path}': {source}")]
    Parse {
        path: String,
        source: toml::de::Error,
    },
    #[error("invalid config: {0}")]
    Invalid(String),
}

fn default_listen() -> String {
    DEFAULT_LISTEN.to_string()
}

fn default_models() -> Vec<ModelRoute> {
    vec![
        ModelRoute {
            name: "fast".to_string(),
            upstream: DEFAULT_FAST_UPSTREAM.to_string(),
            kind: Some(ModelKind::Fast),
            min_complexity: None,
            max_complexity: Some(2),
            health_url: None,
        },
        ModelRoute {
            name: "code".to_string(),
            upstream: DEFAULT_CODE_UPSTREAM.to_string(),
            kind: Some(ModelKind::Code),
            min_complexity: None,
            max_complexity: None,
            health_url: None,
        },
        ModelRoute {
            name: "reasoning".to_string(),
            upstream: DEFAULT_REASONING_UPSTREAM.to_string(),
            kind: Some(ModelKind::Reasoning),
            min_complexity: Some(3),
            max_complexity: None,
            health_url: None,
        },
    ]
}