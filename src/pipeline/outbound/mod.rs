pub mod ner_engine;
pub mod regex_engine;

use crate::config::Config;
use crate::utils::json_walker::StringProcessor;
use crate::vault::Vault;
use ner_engine::NerEngine;
use regex_engine::RegexEngine;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub struct OutboundPipeline {
    pub vault: Arc<Vault>,
    pub regex_engine: Option<RegexEngine>,
    #[cfg(feature = "ner")]
    pub ner_engine: Option<NerEngine>,
}

impl OutboundPipeline {
    pub fn new(vault: Arc<Vault>, config: &Config) -> Self {
        let regex_engine = if config.enable_regex_tier {
            Some(RegexEngine::new())
        } else {
            None
        };

        #[cfg(feature = "ner")]
        let ner_engine = if config.enable_ner_tier {
            let model_dir = std::env::var("BLINDPIPE_NER_MODEL_PATH")
                .unwrap_or_else(|_| "models".to_string());
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
            #[cfg(feature = "ner")]
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

        #[cfg(feature = "ner")]
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
}

pub struct SessionOutbound<'a> {
    pub pipeline: &'a OutboundPipeline,
    pub session_id: &'a str,
}

impl<'a> StringProcessor for SessionOutbound<'a> {
    fn process<'b>(&'b self, s: &'b str) -> Pin<Box<dyn Future<Output = String> + Send + 'b>> {
        Box::pin(async move { self.pipeline.sanitize_text(s, self.session_id).await })
    }
}
