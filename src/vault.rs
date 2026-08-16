use aho_corasick::{AhoCorasick, AhoCorasickBuilder};
use moka::future::Cache;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

pub struct SessionState {
    pub fwd_map: HashMap<String, String>, // original -> token
    pub rev_map: HashMap<String, String>, // token -> original
    pub counters: HashMap<String, usize>, // label -> count
    pub aho_corasick: Option<AhoCorasick>,
    pub aho_tokens: Vec<String>,
}

impl SessionState {
    pub fn new() -> Self {
        Self {
            fwd_map: HashMap::new(),
            rev_map: HashMap::new(),
            counters: HashMap::new(),
            aho_corasick: None,
            aho_tokens: Vec::new(),
        }
    }

    pub fn rebuild_aho(&mut self) {
        let mut tokens = Vec::with_capacity(self.rev_map.len());
        for k in self.rev_map.keys() {
            tokens.push(k.clone());
        }

        if tokens.is_empty() {
            self.aho_corasick = None;
            self.aho_tokens = tokens;
            return;
        }

        let ac = AhoCorasickBuilder::new()
            .match_kind(aho_corasick::MatchKind::LeftmostLongest)
            .build(&tokens)
            .unwrap();

        self.aho_corasick = Some(ac);
        self.aho_tokens = tokens;
    }
}

pub struct Vault {
    cache: Cache<String, Arc<RwLock<SessionState>>>,
}

impl Vault {
    pub fn new(ttl_seconds: u64) -> Self {
        let cache = Cache::builder()
            .time_to_idle(Duration::from_secs(ttl_seconds))
            .build();
        Self { cache }
    }

    pub async fn get_session(&self, session_id: &str) -> Arc<RwLock<SessionState>> {
        self.cache
            .get_with(session_id.to_string(), async {
                Arc::new(RwLock::new(SessionState::new()))
            })
            .await
    }

    pub async fn tokenize(&self, session_id: &str, original_value: &str, label: &str) -> String {
        let state_lock = self.get_session(session_id).await;
        let mut state = state_lock.write().await;

        if let Some(token) = state.fwd_map.get(original_value) {
            return token.clone();
        }

        let label_upper = label.to_uppercase().replace(' ', "_");
        let count = state.counters.entry(label_upper.clone()).or_insert(0);
        *count += 1;
        let synthetic_token = format!("<{}_{}>", label_upper, count);

        state
            .fwd_map
            .insert(original_value.to_string(), synthetic_token.clone());
        state
            .rev_map
            .insert(synthetic_token.clone(), original_value.to_string());

        state.rebuild_aho();

        synthetic_token
    }

    pub async fn desanitize(&self, session_id: &str, text: &str) -> String {
        if text.is_empty() {
            return text.to_string();
        }

        let state_lock = {
            if let Some(s) = self.cache.get(session_id).await {
                s
            } else {
                return text.to_string();
            }
        };

        let state = state_lock.read().await;

        if let Some(ac) = &state.aho_corasick {
            let mut result = String::new();
            let mut last_end = 0;

            for mat in ac.find_iter(text) {
                result.push_str(&text[last_end..mat.start()]);
                let token = &state.aho_tokens[mat.pattern()];
                if let Some(original) = state.rev_map.get(token) {
                    result.push_str(original);
                } else {
                    result.push_str(&text[mat.start()..mat.end()]); // Fallback
                }
                last_end = mat.end();
            }
            result.push_str(&text[last_end..]);
            result
        } else {
            text.to_string()
        }
    }
}
