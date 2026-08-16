import re
from typing import List, Tuple, Dict

def is_luhn_valid(cc_number: str) -> bool:
    """Validates a credit card number using the Luhn algorithm."""
    digits = [int(c) for c in cc_number if c.isdigit()]
    if not digits:
        return False
    checksum = 0
    reverse_digits = digits[::-1]
    for i, d in enumerate(reverse_digits):
        if i % 2 == 1:
            d *= 2
            if d > 9:
                d -= 9
        checksum += d
    return checksum % 10 == 0

class RegexEngine:
    def __init__(self):
        # Dictionary of label to compiled regex pattern
        self.patterns = {
            "API_KEY_OPENAI": re.compile(r"sk-[a-zA-Z0-9]{48}"),
            "API_KEY_AWS": re.compile(r"(A3T[A-Z0-9]|AKIA|AGPA|AIDA|AROA|AIPA|ANPA|ANVA|ASIA)[A-Z0-9]{16}"),
            "API_KEY_GITHUB": re.compile(r"(ghp|gho|ghu|ghs|ghr)_[a-zA-Z0-9]{36}"),
            "CREDIT_CARD": re.compile(r"\b(?:\d[ -]*?){13,16}\b"),
            "SSN": re.compile(r"\b(?!000|666|9\d{2})\d{3}-(?!00)\d{2}-(?!0000)\d{4}\b"),
            "IPV4_ADDRESS": re.compile(r"\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b"),
            "IPV6_ADDRESS": re.compile(r"(([0-9a-fA-F]{1,4}:){7,7}[0-9a-fA-F]{1,4}|([0-9a-fA-F]{1,4}:){1,7}:|([0-9a-fA-F]{1,4}:){1,6}:[0-9a-fA-F]{1,4}|([0-9a-fA-F]{1,4}:){1,5}(:[0-9a-fA-F]{1,4}){1,2}|([0-9a-fA-F]{1,4}:){1,4}(:[0-9a-fA-F]{1,4}){1,3}|([0-9a-fA-F]{1,4}:){1,3}(:[0-9a-fA-F]{1,4}){1,4}|([0-9a-fA-F]{1,4}:){1,2}(:[0-9a-fA-F]{1,4}){1,5}|[0-9a-fA-F]{1,4}:((:[0-9a-fA-F]{1,4}){1,6})|:((:[0-9a-fA-F]{1,4}){1,7}|:)|fe80:(:[0-9a-fA-F]{0,4}){0,4}%[0-9a-zA-Z]{1,}|::(ffff(:0{1,4}){0,1}:){0,1}((25[0-5]|(2[0-4]|1{0,1}[0-9]){0,1}[0-9])\.){3,3}(25[0-5]|(2[0-4]|1{0,1}[0-9]){0,1}[0-9])|([0-9a-fA-F]{1,4}:){1,4}:((25[0-5]|(2[0-4]|1{0,1}[0-9]){0,1}[0-9])\.){3,3}(25[0-5]|(2[0-4]|1{0,1}[0-9]){0,1}[0-9]))"),
            "UUID": re.compile(r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b")
        }

    def extract(self, text: str) -> List[Dict[str, any]]:
        """
        Extracts entities based on regex.
        Returns a list of dicts: {'label': str, 'text': str, 'start': int, 'end': int}
        """
        entities = []
        for label, pattern in self.patterns.items():
            for match in pattern.finditer(text):
                matched_text = match.group(0)
                
                # Special validation for Credit Cards
                if label == "CREDIT_CARD":
                    if not is_luhn_valid(matched_text):
                        continue
                        
                entities.append({
                    "label": label,
                    "text": matched_text,
                    "start": match.start(),
                    "end": match.end()
                })
        return entities
