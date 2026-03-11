from __future__ import annotations

from abc import ABC, abstractmethod

from app.schemas import DictationMode, TranscriptionResponse


class AsrProvider(ABC):
    name: str

    @abstractmethod
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
        raise NotImplementedError
