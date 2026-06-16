use std::env;
use std::fs;
use std::path::Path;

use serde::Deserialize;

const DEFAULT_CONFIG_PATH: &str = "auraroute.toml";
const DEFAULT_LISTEN: &str = "127.0.0.1:8080";
const DEFAULT_LOCAL_UPSTREAM: &str = "http://localhost:11434/v1/chat/completions";

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_listen")]
    pub listen: String,
    #[serde(default = "default_models")]
    pub models: Vec<ModelRoute>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelRoute {
    pub name: String,
    pub upstream: String,
    pub min_complexity: Option<u8>,
    pub max_complexity: Option<u8>,
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
            models: default_models(),
        };
        config.apply_env_overrides();
        config.validate()?;
        Ok(config)
    }

    pub fn select_model(&self, complexity_score: u8) -> Option<&ModelRoute> {
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
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.listen.trim().is_empty() {
            return Err(ConfigError::Invalid("listen address cannot be empty".to_string()));
        }

        if self.models.is_empty() {
            return Err(ConfigError::Invalid("at least one local model route is required".to_string()));
        }

        for model in &self.models {
            model.validate()?;
        }

        Ok(())
    }
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

        Ok(())
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Read {
        path: String,
        source: std::io::Error,
    },
    Parse {
        path: String,
        source: toml::de::Error,
    },
    Invalid(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Read { path, source } => {
                write!(formatter, "failed to read config '{path}': {source}")
            }
            ConfigError::Parse { path, source } => {
                write!(formatter, "failed to parse config '{path}': {source}")
            }
            ConfigError::Invalid(message) => write!(formatter, "invalid config: {message}"),
        }
    }
}

impl std::error::Error for ConfigError {}

fn default_listen() -> String {
    DEFAULT_LISTEN.to_string()
}

fn default_models() -> Vec<ModelRoute> {
    vec![ModelRoute {
        name: "fast".to_string(),
        upstream: DEFAULT_LOCAL_UPSTREAM.to_string(),
        min_complexity: None,
        max_complexity: None,
    }]
}
