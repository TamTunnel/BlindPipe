use promptveil::vault::Vault;

#[tokio::test]
async fn test_tokenize_and_desanitize() {
    let vault = Vault::new(3600);
    
    let token = vault.tokenize("sess_1", "john.doe@example.com", "EMAIL_ADDRESS").await;
    assert_eq!(token, "<EMAIL_ADDRESS_1>");
    
    let token2 = vault.tokenize("sess_1", "jane.doe@example.com", "EMAIL_ADDRESS").await;
    assert_eq!(token2, "<EMAIL_ADDRESS_2>");
    
    let token_same = vault.tokenize("sess_1", "john.doe@example.com", "EMAIL_ADDRESS").await;
    assert_eq!(token_same, "<EMAIL_ADDRESS_1>");
    
    let original = vault.desanitize("sess_1", "Hello <EMAIL_ADDRESS_1>, meet <EMAIL_ADDRESS_2>").await;
    assert_eq!(original, "Hello john.doe@example.com, meet jane.doe@example.com");
}
