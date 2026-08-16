import time
from collections import OrderedDict
from typing import Dict, Tuple

class SessionVault:
    def __init__(self, max_sessions: int = 10000, ttl_seconds: int = 3600):
        self.max_sessions = max_sessions
        self.ttl_seconds = ttl_seconds
        # Structure: session_id -> {
        #   'last_accessed': timestamp,
        #   'fwd': {original: synthetic},
        #   'rev': {synthetic: original},
        #   'counters': {label: count}
        # }
        self.sessions: OrderedDict[str, dict] = OrderedDict()

    def _evict_if_needed(self):
        # Evict by LRU first if full
        while len(self.sessions) > self.max_sessions:
            self.sessions.popitem(last=False)
            
        # Optional: evict expired. This is lazily done on access or here
        now = time.time()
        expired = [sid for sid, data in self.sessions.items() if now - data['last_accessed'] > self.ttl_seconds]
        for sid in expired:
            if sid in self.sessions:
                del self.sessions[sid]

    def _get_or_create_session(self, session_id: str) -> dict:
        now = time.time()
        if session_id in self.sessions:
            self.sessions.move_to_end(session_id)
            self.sessions[session_id]['last_accessed'] = now
            return self.sessions[session_id]
            
        self._evict_if_needed()
        session_data = {
            'last_accessed': now,
            'fwd': {},
            'rev': {},
            'counters': {}
        }
        self.sessions[session_id] = session_data
        return session_data

    def tokenize(self, session_id: str, original_value: str, label: str) -> str:
        """Returns the synthetic token for the given original value."""
        session_data = self._get_or_create_session(session_id)
        fwd = session_data['fwd']
        
        # Consistent mapping: if we already tokenized this value, return the same token
        if original_value in fwd:
            return fwd[original_value]
            
        # Generate new token
        counters = session_data['counters']
        label_upper = label.upper().replace(" ", "_")
        count = counters.get(label_upper, 0) + 1
        counters[label_upper] = count
        
        synthetic_token = f"<{label_upper}_{count}>"
        
        fwd[original_value] = synthetic_token
        session_data['rev'][synthetic_token] = original_value
        
        return synthetic_token

    def get_reverse_mapping(self, session_id: str) -> Dict[str, str]:
        """Returns the dictionary mapping synthetic tokens back to original values."""
        if session_id not in self.sessions:
            return {}
        
        now = time.time()
        if now - self.sessions[session_id]['last_accessed'] > self.ttl_seconds:
            del self.sessions[session_id]
            return {}
            
        self.sessions.move_to_end(session_id)
        self.sessions[session_id]['last_accessed'] = now
        return self.sessions[session_id]['rev']
