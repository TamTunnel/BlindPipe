import os
from gliner import GLiNER
import logging

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

def main():
    model_name = "urchade/gliner_multi_pii-v1"
    save_path = "/app/models/gliner"
    
    logger.info(f"Downloading GLiNER model: {model_name} to {save_path}")
    os.makedirs(save_path, exist_ok=True)
    
    # This downloads and caches the model to the huggingface cache
    model = GLiNER.from_pretrained(model_name)
    
    # Save it explicitly to our local path
    model.save_pretrained(save_path)
    logger.info("Download complete.")

if __name__ == "__main__":
    main()
