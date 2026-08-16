import yaml
from pathlib import Path
from pydantic import BaseModel
from pydantic_settings import BaseSettings

class ServerConfig(BaseModel):
    host: str = "0.0.0.0"
    port: int = 8080
    workers: int = 2

class UpstreamConfig(BaseModel):
    default_base_url: str = "https://api.openai.com"
    timeout_seconds: float = 60.0

class EngineConfig(BaseModel):
    model_name: str = "urchade/gliner_multi_pii-v1"
    ner_threshold: float = 0.45
    labels: list[str] = [
        "person",
        "email address",
        "phone number",
        "organization",
        "address"
    ]
    enable_regex_tier: bool = True

class VaultConfig(BaseModel):
    session_ttl_seconds: int = 3600
    max_sessions: int = 10000

class Settings(BaseSettings):
    server: ServerConfig = ServerConfig()
    upstream: UpstreamConfig = UpstreamConfig()
    engine: EngineConfig = EngineConfig()
    vault: VaultConfig = VaultConfig()

    @classmethod
    def load_config(cls, config_path: str = "config.yaml") -> "Settings":
        path = Path(config_path)
        if path.exists():
            with open(path, "r") as f:
                config_data = yaml.safe_load(f)
            return cls(**config_data)
        return cls()

settings = Settings.load_config()
