pub mod ner_engine;
pub mod regex_engine;

use crate::config::Config;
use crate::vault::Vault;
use ner_engine::NerEngine;
use regex_engine::RegexEngine;
use serde_json::Value;
use std::sync::Arc;

pub struct Sanitizer {
    pub vault: Arc<Vault>,
    pub regex_engine: Option<RegexEngine>,
    pub ner_engine: Option<NerEngine>,
}

impl Sanitizer {
    pub fn new(vault: Arc<Vault>, config: &Config) -> Self {
        let regex_engine = if config.enable_regex_tier {
            Some(RegexEngine::new())
        } else {
            None
        };

        let ner_engine = if config.enable_ner_tier {
            let model_dir =
                std::env::var("GLINER_MODEL_PATH").unwrap_or_else(|_| "models".to_string());
            match NerEngine::new(&model_dir, config.ner_threshold) {
                Ok(engine) => Some(engine),
                Err(e) => {
                    tracing::warn!("NER engine unavailable ({}). Running with regex tier.", e);
                    None
                }
            }
        } else {
            None
        };

        Self {
            vault,
            regex_engine,
            ner_engine,
        }
    }

    pub async fn sanitize_text(&self, text: &str, session_id: &str) -> String {
        if text.is_empty() {
            return text.to_string();
        }

        let mut spans = Vec::new();

        if let Some(regex) = &self.regex_engine {
            let entities = regex.extract(text);
            for e in entities {
                spans.push((e.start, e.end, e.label, e.text));
            }
        }

        if let Some(ner) = &self.ner_engine {
            if let Ok(entities) = ner.extract(text) {
                for e in entities {
                    let overlap = spans.iter().any(|(rs, re, _, _)| {
                        std::cmp::max(e.start, *rs) < std::cmp::min(e.end, *re)
                    });
                    if !overlap {
                        spans.push((e.start, e.end, e.label, e.text));
                    }
                }
            }
        }

        // Sort descending by start to replace without offset shifting
        spans.sort_by(|a, b| b.0.cmp(&a.0));

        let mut result = text.to_string();
        for (start, end, label, matched_text) in spans {
            let token = self
                .vault
                .tokenize(session_id, &matched_text, &label)
                .await;
            result.replace_range(start..end, &token);
        }

        result
    }

    pub async fn walk_and_sanitize(&self, value: &mut Value, session_id: &str) {
        match value {
            Value::String(s) => {
                let sanitized = self.sanitize_text(s, session_id).await;
                *s = sanitized;
            }
            Value::Array(arr) => {
                for item in arr.iter_mut() {
                    Box::pin(self.walk_and_sanitize(item, session_id)).await;
                }
            }
            Value::Object(obj) => {
                for (_, val) in obj.iter_mut() {
                    Box::pin(self.walk_and_sanitize(val, session_id)).await;
                }
            }
            _ => {}
        }
    }

    pub async fn walk_and_desanitize(&self, value: &mut Value, session_id: &str) {
        match value {
            Value::String(s) => {
                let desanitized = self.vault.desanitize(session_id, s).await;
                *s = desanitized;
            }
            Value::Array(arr) => {
                for item in arr.iter_mut() {
                    Box::pin(self.walk_and_desanitize(item, session_id)).await;
                }
            }
            Value::Object(obj) => {
                for (_, val) in obj.iter_mut() {
                    Box::pin(self.walk_and_desanitize(val, session_id)).await;
                }
            }
            _ => {}
        }
    }
}
