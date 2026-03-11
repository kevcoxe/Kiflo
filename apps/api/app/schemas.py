from __future__ import annotations

from enum import StrEnum

from pydantic import BaseModel, Field


class DictationMode(StrEnum):
    DEFAULT = "default"
    EMAIL = "email"
    CHAT = "chat"
    NOTES = "notes"
    BULLET_LIST = "bullet_list"


class HealthResponse(BaseModel):
    status: str = "ok"
    asr_provider: str
    cleanup_provider: str


class TranscriptionResponse(BaseModel):
    raw_text: str
    language: str = "en"
    duration_ms: int = Field(ge=0)
    provider: str


class CleanupRequest(BaseModel):
    raw_text: str = Field(min_length=1)
    mode: DictationMode = DictationMode.DEFAULT
    app_name: str | None = None
    preserve_raw: bool = False


class CleanupResponse(BaseModel):
    clean_text: str
    provider: str
