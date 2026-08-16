use blindpipe::config::Config;
use blindpipe::pipeline::outbound::{OutboundPipeline, SessionOutbound};
use blindpipe::pipeline::inbound::{InboundPipeline, SessionInbound};
use blindpipe::utils::json_walker::walk_json;
use blindpipe::vault::Vault;
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn test_sanitizer_json_walk_and_desanitize() {
    let mut config = Config::load();
    config.enable_regex_tier = true;
    config.enable_ner_tier = false; // Disable NER in unit test to avoid needing local model download

    let vault = Arc::new(Vault::new(3600));
    let outbound = OutboundPipeline::new(vault.clone(), &config);
    let inbound = InboundPipeline::new(vault.clone());

    let session_id = "test-session-123";

    let mut payload = json!({
        "model": "gpt-4",
        "messages": [
            {
                "role": "user",
                "content": "My IP is 192.168.1.50 and my SSN is 123-45-6789."
            }
        ]
    });

    // 1. Sanitize payload
    let processor_out = SessionOutbound {
        pipeline: &outbound,
        session_id,
    };
    walk_json(&mut payload, &processor_out).await;

    let content = payload["messages"][0]["content"].as_str().unwrap();
    assert!(content.contains("<IPV4_ADDRESS_1>"));
    assert!(content.contains("<SSN_1>"));
    assert!(!content.contains("192.168.1.50"));
    assert!(!content.contains("123-45-6789"));

    // 2. Simulate AI response using the tokens
    let mut ai_response = json!({
        "id": "chatcmpl-123",
        "choices": [
            {
                "message": {
                    "role": "assistant",
                    "content": "Received data for IP <IPV4_ADDRESS_1> and user with SSN <SSN_1>."
                }
            }
        ]
    });

    // 3. Desanitize AI response
    let processor_in = SessionInbound {
        pipeline: &inbound,
        session_id,
    };
    walk_json(&mut ai_response, &processor_in).await;

    let reply_content = ai_response["choices"][0]["message"]["content"].as_str().unwrap();
    assert_eq!(
        reply_content,
        "Received data for IP 192.168.1.50 and user with SSN 123-45-6789."
    );
}
