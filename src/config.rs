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
            server_port: env::var("BLINDPIPE_PORT")
                .or_else(|_| env::var("PORT"))
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .unwrap_or(8080),
            upstream_base_url: env::var("BLINDPIPE_UPSTREAM_URL")
                .or_else(|_| env::var("UPSTREAM_BASE_URL"))
                .unwrap_or_else(|_| "https://api.openai.com".to_string()),
            ner_threshold: env::var("BLINDPIPE_NER_THRESHOLD")
                .unwrap_or_else(|_| "0.45".to_string())
                .parse()
                .unwrap_or(0.45),
            session_ttl_seconds: env::var("BLINDPIPE_SESSION_TTL")
                .unwrap_or_else(|_| "3600".to_string())
                .parse()
                .unwrap_or(3600),
            enable_regex_tier: env::var("BLINDPIPE_ENABLE_REGEX")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            enable_ner_tier: env::var("BLINDPIPE_ENABLE_NER")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
        }
    }
}
