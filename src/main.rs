use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::any,
    Router,
};
use bytes::Bytes;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::Value;
use std::sync::Arc;
use tokio::net::TcpListener;
use uuid::Uuid;

use blindpipe::config::Config;
use blindpipe::pipeline::inbound::{InboundPipeline, SessionInbound};
use blindpipe::pipeline::outbound::{OutboundPipeline, SessionOutbound};
use blindpipe::utils::json_walker::walk_json;
use blindpipe::utils::sse_buffer::SseBuffer;
use blindpipe::vault::Vault;

#[derive(Clone)]
struct AppState {
    outbound: Arc<OutboundPipeline>,
    inbound: Arc<InboundPipeline>,
    client: Client,
    config: Arc<Config>,
}

async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

pub fn resolve_upstream_target(
    req_path: &str,
    headers: &HeaderMap,
    default_base_url: &str,
) -> (String, String) {
    // 1. Highest Priority: Explicit header override
    if let Some(header_url) = headers
        .get("x-upstream-base-url")
        .and_then(|v| v.to_str().ok())
    {
        return (header_url.trim_end_matches('/').to_string(), req_path.to_string());
    }

    // 2. Second Priority: Path-based provider prefixes (strips prefix from downstream path)
    let known_prefixes = [
        ("/openrouter", "https://openrouter.ai/api"),
        ("/anthropic", "https://api.anthropic.com"),
        ("/openai", "https://api.openai.com"),
        ("/gemini", "https://generativelanguage.googleapis.com"),
        ("/ollama", "http://localhost:11434"),
    ];

    for (prefix, base_url) in known_prefixes {
        if req_path.starts_with(prefix) {
            let stripped_path = &req_path[prefix.len()..];
            let clean_path = if stripped_path.is_empty() {
                "/"
            } else if !stripped_path.starts_with('/') && !stripped_path.starts_with('?') {
                stripped_path
            } else {
                stripped_path
            };
            let final_path = if clean_path.starts_with('/') {
                clean_path.to_string()
            } else {
                format!("/{}", clean_path)
            };
            return (base_url.trim_end_matches('/').to_string(), final_path);
        }
    }

    // 3. Fallback: Default configured upstream
    (default_base_url.trim_end_matches('/').to_string(), req_path.to_string())
}

async fn proxy_handler(
    State(state): State<AppState>,
    req: Request,
) -> Result<Response, StatusCode> {
    let session_id = req
        .headers()
        .get("X-Session-ID")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("sess_{}", Uuid::new_v4().simple()));

    let raw_path = req.uri().path_and_query().map(|x| x.as_str()).unwrap_or("/");
    let (upstream_base, forwarded_path) = resolve_upstream_target(raw_path, req.headers(), &state.config.upstream_base_url);
    let upstream_url = format!("{}{}", upstream_base, forwarded_path);

    let method = req.method().clone();

    // Extract headers (filter out hop-by-hop and internal override headers)
    let mut headers = HeaderMap::new();
    for (k, v) in req.headers() {
        if k != "host" && k != "content-length" && k != "x-upstream-base-url" {
            headers.insert(k.clone(), v.clone());
        }
    }

    // Process body if JSON
    let content_type = req
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let body_bytes = axum::body::to_bytes(req.into_body(), usize::MAX)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let mut body_vec = body_bytes.to_vec();

    if content_type.contains("application/json") && !body_vec.is_empty() {
        if let Ok(mut json_val) = serde_json::from_slice::<Value>(&body_vec) {
            let processor = SessionOutbound {
                pipeline: &state.outbound,
                session_id: &session_id,
            };
            walk_json(&mut json_val, &processor).await;
            if let Ok(new_body) = serde_json::to_vec(&json_val) {
                body_vec = new_body;
            }
        }
    }

    let mut req_builder = state.client.request(method, &upstream_url).headers(headers);
    if !body_vec.is_empty() {
        req_builder = req_builder.body(body_vec);
    }

    let upstream_res = match req_builder.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Upstream error: {}", e);
            return Err(StatusCode::BAD_GATEWAY);
        }
    };

    let mut resp_builder = Response::builder().status(upstream_res.status());
    for (k, v) in upstream_res.headers() {
        if k != "content-length" && k != "transfer-encoding" && k != "content-encoding" {
            resp_builder = resp_builder.header(k, v);
        }
    }
    resp_builder = resp_builder.header(
        "X-Session-ID",
        HeaderValue::from_str(&session_id).unwrap_or_else(|_| HeaderValue::from_static("")),
    );

    let res_content_type = upstream_res
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    if res_content_type.contains("text/event-stream") {
        let session_id_clone = session_id.clone();
        let inbound_clone = state.inbound.clone();

        let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::convert::Infallible>>(32);
        
        tokio::spawn(async move {
            let mut stream = upstream_res.bytes_stream();
            let mut sse_buffer = SseBuffer::new();

            while let Some(result) = stream.next().await {
                if let Ok(bytes) = result {
                    sse_buffer.extend(&bytes);
                    while let Some(event_text) = sse_buffer.next_event() {
                        let mut final_text = String::new();
                        let part = event_text.trim_end_matches('\n');
                        if part.starts_with("data: ") {
                            let data_str = &part[6..];
                            if data_str.trim() == "[DONE]" {
                                final_text.push_str(&format!("data: {}\n\n", data_str));
                            } else if let Ok(mut json_val) = serde_json::from_str::<Value>(data_str) {
                                let processor = SessionInbound {
                                    pipeline: &inbound_clone,
                                    session_id: &session_id_clone,
                                };
                                walk_json(&mut json_val, &processor).await;
                                let new_data = serde_json::to_string(&json_val).unwrap();
                                final_text.push_str(&format!("data: {}\n\n", new_data));
                            } else {
                                let processor = SessionInbound {
                                    pipeline: &inbound_clone,
                                    session_id: &session_id_clone,
                                };
                                let desanitized = blindpipe::utils::json_walker::StringProcessor::process(&processor, data_str).await;
                                final_text.push_str(&format!("data: {}\n\n", desanitized));
                            }
                        } else if !part.is_empty() {
                            final_text.push_str(&format!("{}\n\n", part));
                        }

                        if !final_text.is_empty() {
                            let _ = tx.send(Ok(Bytes::from(final_text))).await;
                        }
                    }
                }
            }
        });

        let body = Body::from_stream(tokio_stream::wrappers::ReceiverStream::new(rx));
        Ok(resp_builder.body(body).unwrap())
    } else {
        let resp_bytes = upstream_res
            .bytes()
            .await
            .map_err(|_| StatusCode::BAD_GATEWAY)?;
        let mut resp_vec = resp_bytes.to_vec();

        if res_content_type.contains("application/json") && !resp_vec.is_empty() {
            if let Ok(mut json_val) = serde_json::from_slice::<Value>(&resp_vec) {
                let processor = SessionInbound {
                    pipeline: &state.inbound,
                    session_id: &session_id,
                };
                walk_json(&mut json_val, &processor).await;
                if let Ok(new_body) = serde_json::to_vec(&json_val) {
                    resp_vec = new_body;
                }
            }
        } else if res_content_type.contains("image/") || 
                  res_content_type.contains("application/pdf") || 
                  res_content_type.contains("application/vnd.openxmlformats") || 
                  res_content_type.contains("application/epub+zip") {
            resp_vec = blindpipe::pipeline::inbound::metadata::MetadataStripper::strip_binary(&resp_vec, &res_content_type);
        } else if !resp_vec.is_empty() {
            if let Ok(text) = String::from_utf8(resp_vec.clone()) {
                // Just use the pipeline manually
                let processor = SessionInbound {
                    pipeline: &state.inbound,
                    session_id: &session_id,
                };
                let desanitized = blindpipe::utils::json_walker::StringProcessor::process(&processor, &text).await;
                resp_vec = desanitized.into_bytes();
            }
        }

        Ok(resp_builder.body(Body::from(resp_vec)).unwrap())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = Arc::new(Config::load());
    let vault = Arc::new(Vault::new(config.session_ttl_seconds));
    
    let outbound = Arc::new(OutboundPipeline::new(vault.clone(), &config));
    let inbound = Arc::new(InboundPipeline::new(vault.clone()));

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs_f32(60.0))
        .build()?;

    let app_state = AppState {
        outbound,
        inbound,
        client,
        config: config.clone(),
    };

    let app = Router::new()
        .route("/healthz", axum::routing::get(health_check))
        .route("/*path", any(proxy_handler))
        .with_state(app_state);

    let addr = format!("0.0.0.0:{}", config.server_port);
    tracing::info!("Listening on {}", addr);
    let listener = TcpListener::bind(addr).await?;

    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    #[test]
    fn test_default_fallback() {
        let headers = HeaderMap::new();
        let (base, path) = resolve_upstream_target("/v1/chat/completions", &headers, "https://api.openai.com");
        assert_eq!(base, "https://api.openai.com");
        assert_eq!(path, "/v1/chat/completions");
    }

    #[test]
    fn test_header_override() {
        let mut headers = HeaderMap::new();
        headers.insert("x-upstream-base-url", "https://custom.provider.com/api/".parse().unwrap());
        let (base, path) = resolve_upstream_target("/v1/models", &headers, "https://api.openai.com");
        assert_eq!(base, "https://custom.provider.com/api");
        assert_eq!(path, "/v1/models");
    }

    #[test]
    fn test_path_prefixes() {
        let headers = HeaderMap::new();
        
        let (base, path) = resolve_upstream_target("/openrouter/v1/chat/completions", &headers, "https://api.openai.com");
        assert_eq!(base, "https://openrouter.ai/api");
        assert_eq!(path, "/v1/chat/completions");

        let (base, path) = resolve_upstream_target("/anthropic/v1/messages?beta=true", &headers, "https://api.openai.com");
        assert_eq!(base, "https://api.anthropic.com");
        assert_eq!(path, "/v1/messages?beta=true");

        let (base, path) = resolve_upstream_target("/openai/v1/models", &headers, "https://api.openai.com");
        assert_eq!(base, "https://api.openai.com");
        assert_eq!(path, "/v1/models");

        let (base, path) = resolve_upstream_target("/gemini/v1beta/models", &headers, "https://api.openai.com");
        assert_eq!(base, "https://generativelanguage.googleapis.com");
        assert_eq!(path, "/v1beta/models");

        let (base, path) = resolve_upstream_target("/ollama/api/generate", &headers, "https://api.openai.com");
        assert_eq!(base, "http://localhost:11434");
        assert_eq!(path, "/api/generate");

        let (base, path) = resolve_upstream_target("/openrouter", &headers, "https://api.openai.com");
        assert_eq!(base, "https://openrouter.ai/api");
        assert_eq!(path, "/");
    }
}

