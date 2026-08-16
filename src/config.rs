use serde::Deserialize;
use std::env;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server_port: u16,
    pub upstream_base_url: String,
    pub ner_threshold: f32,
    pub session_ttl_seconds: u64,
    pub enable_regex_tier: bool,
    pub enable_ner_tier: bool,
}

impl Config {
    pub fn load() -> Self {
        Self {
            server_port: env::var("SERVER_PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .unwrap_or(8080),
            upstream_base_url: env::var("UPSTREAM_BASE_URL")
                .unwrap_or_else(|_| "https://api.openai.com".to_string()),
            ner_threshold: env::var("NER_THRESHOLD")
                .unwrap_or_else(|_| "0.45".to_string())
                .parse()
                .unwrap_or(0.45),
            session_ttl_seconds: env::var("SESSION_TTL_SECONDS")
                .unwrap_or_else(|_| "3600".to_string())
                .parse()
                .unwrap_or(3600),
            enable_regex_tier: env::var("ENABLE_REGEX_TIER")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            enable_ner_tier: env::var("ENABLE_NER_TIER")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
        }
    }
}
