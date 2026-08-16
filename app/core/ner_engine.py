import logging
from gliner import GLiNER
from typing import List, Dict
import os

logger = logging.getLogger(__name__)

class NEREngine:
    def __init__(self, model_name: str, labels: List[str], threshold: float = 0.45):
        self.labels = labels
        self.threshold = threshold
        
        # Determine local path for offline loading
        # During docker build, we download it to /app/models/gliner
        local_path = os.environ.get("GLINER_MODEL_PATH", "/app/models/gliner")
        
        try:
            if os.path.exists(local_path) and os.listdir(local_path):
                logger.info(f"Loading GLiNER model from local path: {local_path}")
                self.model = GLiNER.from_pretrained(local_path, local_files_only=True)
            else:
                logger.warning(f"Local path {local_path} empty or not found. Downloading {model_name}...")
                self.model = GLiNER.from_pretrained(model_name)
        except Exception as e:
            logger.error(f"Failed to load GLiNER model: {e}")
            raise e

    def extract(self, text: str) -> List[Dict[str, any]]:
        """
        Extracts entities using GLiNER.
        Returns a list of dicts: {'label': str, 'text': str, 'start': int, 'end': int}
        """
        if not text.strip():
            return []
            
        entities = self.model.predict_entities(text, self.labels, threshold=self.threshold)
        
        formatted_entities = []
        for ent in entities:
            formatted_entities.append({
                "label": ent["label"].upper().replace(" ", "_"),
                "text": ent["text"],
                "start": ent["start"],
                "end": ent["end"]
            })
            
        return formatted_entities
