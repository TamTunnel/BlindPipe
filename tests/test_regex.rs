use blindpipe::pipeline::outbound::regex_engine::{RegexEngine, is_luhn_valid};

#[test]
fn test_luhn_validation() {
    assert!(is_luhn_valid("4242424242424242"));
    assert!(!is_luhn_valid("4242424242424243"));
}

#[test]
fn test_regex_extraction() {
    let engine = RegexEngine::new();
    let text = "Here is an IP 192.168.1.1 and a key sk-123456789012345678901234567890123456789012345678";
    
    let entities = engine.extract(text);
    assert_eq!(entities.len(), 2);
    
    let ips: Vec<_> = entities.iter().filter(|e| e.label == "IPV4_ADDRESS").collect();
    assert_eq!(ips.len(), 1);
    assert_eq!(ips[0].text, "192.168.1.1");
    
    let keys: Vec<_> = entities.iter().filter(|e| e.label == "API_KEY_OPENAI").collect();
    assert_eq!(keys.len(), 1);
    assert!(keys[0].text.starts_with("sk-"));
}
