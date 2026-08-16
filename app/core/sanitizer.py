import ahocorasick
from typing import List, Dict, Optional
from app.core.regex_engine import RegexEngine
from app.core.ner_engine import NEREngine
from app.core.vault import SessionVault
from app.config import settings
import logging

logger = logging.getLogger(__name__)

class Sanitizer:
    def __init__(self, vault: SessionVault):
        self.vault = vault
        self.regex_engine = RegexEngine() if settings.engine.enable_regex_tier else None
        
        try:
            self.ner_engine = NEREngine(
                model_name=settings.engine.model_name,
                labels=settings.engine.labels,
                threshold=settings.engine.ner_threshold
            )
        except Exception as e:
            logger.error(f"Could not initialize NER Engine: {e}. Running in degraded mode (Regex only if enabled).")
            self.ner_engine = None

    def sanitize_text(self, text: str, session_id: str) -> str:
        """
        Orchestrates Regex + NER passes over the text and replaces PII with tokens.
        """
        if not text or not isinstance(text, str):
            return text
            
        entities = []
        
        # 1. Regex Pass (Tier 1)
        if self.regex_engine:
            entities.extend(self.regex_engine.extract(text))
            
        # 2. NER Pass (Tier 2)
        if self.ner_engine:
            ner_entities = self.ner_engine.extract(text)
            
            # Merge logic: Prioritize regex. Discard NER if it overlaps with Regex.
            for ner_ent in ner_entities:
                overlap = False
                for reg_ent in entities:
                    # Check if [start_1, end_1] overlaps with [start_2, end_2]
                    if max(ner_ent['start'], reg_ent['start']) < min(ner_ent['end'], reg_ent['end']):
                        overlap = True
                        break
                if not overlap:
                    entities.append(ner_ent)
                    
        # 3. Sort & Replace (Reverse order to avoid shifting indices)
        entities.sort(key=lambda x: x['start'], reverse=True)
        
        result_text = text
        for ent in entities:
            token = self.vault.tokenize(session_id, ent['text'], ent['label'])
            start, end = ent['start'], ent['end']
            result_text = result_text[:start] + token + result_text[end:]
            
        return result_text

    def desanitize_text(self, text: str, session_id: str) -> str:
        """
        Replaces synthetic tokens with original values using Aho-Corasick.
        """
        if not text or not isinstance(text, str):
            return text
            
        rev_mapping = self.vault.get_reverse_mapping(session_id)
        if not rev_mapping:
            return text
            
        # Build Aho-Corasick automaton
        A = ahocorasick.Automaton()
        for idx, (token, original) in enumerate(rev_mapping.items()):
            A.add_word(token, (idx, token, original))
        A.make_automaton()
        
        # We need to collect replacements and apply them carefully
        replacements = []
        for end_idx, (idx, token, original) in A.iter(text):
            start_idx = end_idx - len(token) + 1
            replacements.append((start_idx, end_idx + 1, token, original))
            
        # If overlapping replacements exist, we prioritize the longest/first.
        # But tokens typically don't overlap. We sort by start reverse.
        replacements.sort(key=lambda x: x[0], reverse=True)
        
        result_text = text
        # To handle overlapping or nested tokens cleanly, keep track of replaced bounds
        last_start = len(result_text)
        
        for start, end, token, original in replacements:
            if end <= last_start:
                result_text = result_text[:start] + original + result_text[end:]
                last_start = start
                
        return result_text
