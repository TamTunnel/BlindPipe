import pytest
import time
from app.core.vault import SessionVault

def test_session_vault_tokenization():
    vault = SessionVault()
    session_id = "test_session_1"
    
    token1 = vault.tokenize(session_id, "John Doe", "PERSON")
    assert token1 == "<PERSON_1>"
    
    token2 = vault.tokenize(session_id, "Jane Doe", "PERSON")
    assert token2 == "<PERSON_2>"
    
    # Same value should return same token
    token3 = vault.tokenize(session_id, "John Doe", "PERSON")
    assert token3 == "<PERSON_1>"

def test_session_vault_reverse_mapping():
    vault = SessionVault()
    session_id = "test_session_2"
    
    vault.tokenize(session_id, "john@example.com", "EMAIL_ADDRESS")
    vault.tokenize(session_id, "jane@example.com", "EMAIL_ADDRESS")
    
    rev_map = vault.get_reverse_mapping(session_id)
    assert rev_map["<EMAIL_ADDRESS_1>"] == "john@example.com"
    assert rev_map["<EMAIL_ADDRESS_2>"] == "jane@example.com"

def test_session_vault_ttl_eviction():
    vault = SessionVault(ttl_seconds=1)
    session_id = "test_session_3"
    
    vault.tokenize(session_id, "Secret", "ORGANIZATION")
    
    # Initially there is a mapping
    assert "<ORGANIZATION_1>" in vault.get_reverse_mapping(session_id)
    
    # Wait for TTL
    time.sleep(1.1)
    
    # Mapping should be empty due to expiration
    assert vault.get_reverse_mapping(session_id) == {}
