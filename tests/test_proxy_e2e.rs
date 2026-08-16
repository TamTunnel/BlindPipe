use promptveil::config::Config;
use promptveil::sanitizer::Sanitizer;
use promptveil::vault::Vault;
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn test_sanitizer_json_walk_and_desanitize() {
    let mut config = Config::load();
    config.enable_regex_tier = true;
    config.enable_ner_tier = false; // Disable NER in unit test to avoid needing local model download

    let vault = Arc::new(Vault::new(3600));
    let sanitizer = Sanitizer::new(vault.clone(), &config);

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
    sanitizer.walk_and_sanitize(&mut payload, session_id).await;

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
    sanitizer.walk_and_desanitize(&mut ai_response, session_id).await;

    let reply_content = ai_response["choices"][0]["message"]["content"].as_str().unwrap();
    assert_eq!(
        reply_content,
        "Received data for IP 192.168.1.50 and user with SSN 123-45-6789."
    );
}
