from __future__ import annotations

from fastapi import Depends

from app.config import Settings, get_settings
from app.providers.asr import AsrProvider
from app.providers.cleanup import CleanupProvider
from app.providers.mock import MockAsrProvider, MockCleanupProvider
from app.providers.openai_compatible import (
    OpenAICompatibleAsrProvider,
    OpenAICompatibleCleanupProvider,
)


def build_asr_provider(settings: Settings) -> AsrProvider:
    if settings.asr_provider == "mock":
        return MockAsrProvider()
    if settings.asr_provider == "openai_compatible":
        return OpenAICompatibleAsrProvider()
    raise ValueError(f"Unsupported ASR provider: {settings.asr_provider}")


def build_cleanup_provider(settings: Settings) -> CleanupProvider:
    if settings.cleanup_provider == "mock":
        return MockCleanupProvider()
    if settings.cleanup_provider == "openai_compatible":
        return OpenAICompatibleCleanupProvider()
    raise ValueError(f"Unsupported cleanup provider: {settings.cleanup_provider}")


def get_asr_provider(settings: Settings = Depends(get_settings)) -> AsrProvider:
    return build_asr_provider(settings)


def get_cleanup_provider(settings: Settings = Depends(get_settings)) -> CleanupProvider:
    return build_cleanup_provider(settings)
