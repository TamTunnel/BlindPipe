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
use promptveil::config::Config;
use promptveil::sanitizer::Sanitizer;
use promptveil::vault::Vault;
use reqwest::Client;
use serde_json::Value;
use std::sync::Arc;
use tokio::net::TcpListener;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    sanitizer: Arc<Sanitizer>,
    client: Client,
    config: Arc<Config>,
}

async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "ok")
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

    let path = req.uri().path().to_string();
    let query = req.uri().query().map(|q| format!("?{}", q)).unwrap_or_default();
    let upstream_url = format!("{}{}{}", state.config.upstream_base_url, path, query);

    let method = req.method().clone();

    // Extract headers (filter out hop-by-hop)
    let mut headers = HeaderMap::new();
    for (k, v) in req.headers() {
        if k != "host" && k != "content-length" {
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
            state
                .sanitizer
                .walk_and_sanitize(&mut json_val, &session_id)
                .await;
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
        let sanitizer_clone = state.sanitizer.clone();

        let stream = upstream_res.bytes_stream();
        let mapped_stream = stream.then(move |result| {
            let session_id = session_id_clone.clone();
            let sanitizer = sanitizer_clone.clone();
            async move {
                match result {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes).to_string();
                        let mut final_text = String::new();

                        for part in text.split("\n\n") {
                            if part.is_empty() {
                                continue;
                            }
                            if part.starts_with("data: ") {
                                let data_str = &part[6..];
                                if data_str.trim() == "[DONE]" {
                                    final_text.push_str(&format!("data: {}\n\n", data_str));
                                    continue;
                                }

                                if let Ok(mut json_val) = serde_json::from_str::<Value>(data_str) {
                                    sanitizer
                                        .walk_and_desanitize(&mut json_val, &session_id)
                                        .await;
                                    let new_data = serde_json::to_string(&json_val).unwrap();
                                    final_text.push_str(&format!("data: {}\n\n", new_data));
                                } else {
                                    let desanitized =
                                        sanitizer.vault.desanitize(&session_id, data_str).await;
                                    final_text.push_str(&format!("data: {}\n\n", desanitized));
                                }
                            } else {
                                final_text.push_str(&format!("{}\n\n", part));
                            }
                        }
                        Ok::<_, std::convert::Infallible>(Bytes::from(final_text))
                    }
                    Err(_) => Ok::<_, std::convert::Infallible>(Bytes::new()),
                }
            }
        });

        let body = Body::from_stream(mapped_stream);
        Ok(resp_builder.body(body).unwrap())
    } else {
        let resp_bytes = upstream_res
            .bytes()
            .await
            .map_err(|_| StatusCode::BAD_GATEWAY)?;
        let mut resp_vec = resp_bytes.to_vec();

        if res_content_type.contains("application/json") && !resp_vec.is_empty() {
            if let Ok(mut json_val) = serde_json::from_slice::<Value>(&resp_vec) {
                state
                    .sanitizer
                    .walk_and_desanitize(&mut json_val, &session_id)
                    .await;
                if let Ok(new_body) = serde_json::to_vec(&json_val) {
                    resp_vec = new_body;
                }
            }
        } else if !resp_vec.is_empty() {
            if let Ok(text) = String::from_utf8(resp_vec.clone()) {
                let desanitized = state.sanitizer.vault.desanitize(&session_id, &text).await;
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
    let sanitizer = Arc::new(Sanitizer::new(vault.clone(), &config));

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs_f32(60.0))
        .build()?;

    let app_state = AppState {
        sanitizer,
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
