from functools import lru_cache
from pathlib import Path

from pydantic import Field
from pydantic_settings import BaseSettings, SettingsConfigDict

API_ENV_FILE = Path(__file__).resolve().parents[1] / ".env"


class Settings(BaseSettings):
    model_config = SettingsConfigDict(
        env_file=API_ENV_FILE,
        env_prefix="TATOR_",
        extra="ignore",
    )

    app_name: str = "tator-api"
    environment: str = "development"
    cors_allowed_origins: list[str] = Field(
        default_factory=lambda: [
            "http://127.0.0.1:1420",
            "http://localhost:1420",
        ]
    )
    asr_provider: str = Field(default="mock")
    cleanup_provider: str = Field(default="mock")
    mlx_whisper_model_repo: str = "mlx-community/whisper-tiny.en-mlx"
    mlx_whisper_language: str = "en"
    mlx_whisper_download_root: str | None = None
    openai_base_url: str | None = None
    openai_api_key: str | None = None
    openai_asr_model: str = "gpt-4o-mini-transcribe"
    openai_cleanup_model: str = "gpt-4.1-mini"


@lru_cache
def get_settings() -> Settings:
    return Settings()
