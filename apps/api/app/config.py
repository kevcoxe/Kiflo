from functools import lru_cache

from pydantic import Field
from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    model_config = SettingsConfigDict(
        env_file=".env",
        env_prefix="TATOR_",
        extra="ignore",
    )

    app_name: str = "tator-api"
    environment: str = "development"
    asr_provider: str = Field(default="mock")
    cleanup_provider: str = Field(default="mock")
    openai_base_url: str | None = None
    openai_api_key: str | None = None
    openai_asr_model: str = "gpt-4o-mini-transcribe"
    openai_cleanup_model: str = "gpt-4.1-mini"


@lru_cache
def get_settings() -> Settings:
    return Settings()
