from __future__ import annotations

from app.providers.asr import AsrProvider
from app.providers.cleanup import CleanupProvider
from app.schemas import CleanupResponse, DictationMode, TranscriptionResponse


class ProviderNotConfiguredError(RuntimeError):
    """Raised when a provider shell is selected without credentials."""


class OpenAICompatibleAsrProvider(AsrProvider):
    name = "openai_compatible"

    async def transcribe(
        self,
        *,
        audio: bytes,
        filename: str | None,
        content_type: str | None,
        mode: DictationMode,
        app_name: str | None,
        duration_ms: int,
    ) -> TranscriptionResponse:
        raise ProviderNotConfiguredError(
            "The openai_compatible ASR provider is configured as a shell only. "
            "Set credentials and implement the HTTP adapter before enabling it."
        )


class OpenAICompatibleCleanupProvider(CleanupProvider):
    name = "openai_compatible"

    async def cleanup(
        self,
        *,
        raw_text: str,
        mode: DictationMode,
        app_name: str | None,
        preserve_raw: bool,
    ) -> CleanupResponse:
        raise ProviderNotConfiguredError(
            "The openai_compatible cleanup provider is configured as a shell only. "
            "Set credentials and implement the HTTP adapter before enabling it."
        )
