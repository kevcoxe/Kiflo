from __future__ import annotations

from abc import ABC, abstractmethod

from app.schemas import CleanupResponse, DictationMode


class CleanupProvider(ABC):
    name: str

    @abstractmethod
    async def cleanup(
        self,
        *,
        raw_text: str,
        mode: DictationMode,
        app_name: str | None,
        preserve_raw: bool,
    ) -> CleanupResponse:
        raise NotImplementedError
