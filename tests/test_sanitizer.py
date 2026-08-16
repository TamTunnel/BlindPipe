import pytest
from app.core.regex_engine import is_luhn_valid, RegexEngine
from app.core.vault import SessionVault
from app.core.sanitizer import Sanitizer

def test_luhn_validator():
    # Valid Visa test card
    assert is_luhn_valid("4242424242424242") == True
    # Invalid card
    assert is_luhn_valid("4242424242424243") == False

def test_regex_engine_cc():
    engine = RegexEngine()
    text = "My card is 4242424242424242 and another 1234567812345678"
    entities = engine.extract(text)
    
    # 4242... is valid luhn, 1234... is not
    assert len(entities) == 1
    assert entities[0]["label"] == "CREDIT_CARD"
    assert entities[0]["text"] == "4242424242424242"

def test_sanitizer_regex_only():
    vault = SessionVault()
    # Mocking out the NER Engine for pure regex test
    sanitizer = Sanitizer(vault)
    sanitizer.ner_engine = None 
    
    text = "Here is my key sk-123456789012345678901234567890123456789012345678"
    session_id = "test_sess"
    
    masked = sanitizer.sanitize_text(text, session_id)
    assert "sk-123" not in masked
    assert "<API_KEY_OPENAI_1>" in masked
    
    original = sanitizer.desanitize_text(masked, session_id)
    assert original == text
