from __future__ import annotations

import re

from app.providers.asr import AsrProvider
from app.providers.cleanup import CleanupProvider
from app.schemas import CleanupResponse, DictationMode, TranscriptionResponse

FILLER_PATTERN = re.compile(r"\b(?:um+|uh+|erm|ah)\b", re.IGNORECASE)
MULTISPACE_PATTERN = re.compile(r"\s+")
QUESTION_OPENERS = {
    "can",
    "could",
    "would",
    "will",
    "should",
    "is",
    "are",
    "do",
    "does",
    "did",
}

MODE_TRANSCRIPTS = {
    DictationMode.DEFAULT: "hey can you send the update to rebekah tomorrow afternoon",
    DictationMode.CHAT: "running five minutes late be there soon",
    DictationMode.BULLET_LIST: "make a list eggs milk bread and apples",
}


class MockAsrProvider(AsrProvider):
    name = "mock"

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
        transcript = MODE_TRANSCRIPTS[mode]
        if app_name and mode == DictationMode.DEFAULT:
            transcript = f"hey can you send the update to rebekah in {app_name}"

        return TranscriptionResponse(
            raw_text=transcript,
            language="en",
            duration_ms=duration_ms,
            provider=self.name,
        )


class MockCleanupProvider(CleanupProvider):
    name = "mock"

    async def cleanup(
        self,
        *,
        raw_text: str,
        mode: DictationMode,
        app_name: str | None,
        preserve_raw: bool,
    ) -> CleanupResponse:
        text = normalize_text(raw_text, preserve_raw=preserve_raw)
        if mode == DictationMode.BULLET_LIST:
            bullets = format_bullets(text)
            if bullets:
                return CleanupResponse(clean_text=bullets, provider=self.name)

        return CleanupResponse(
            clean_text=format_sentence(text, mode=mode),
            provider=self.name,
        )


def normalize_text(raw_text: str, *, preserve_raw: bool) -> str:
    text = MULTISPACE_PATTERN.sub(" ", raw_text).strip()
    if preserve_raw:
        return text

    text = FILLER_PATTERN.sub(" ", text)
    text = re.sub(r"\b(?:you know)\b", " ", text, flags=re.IGNORECASE)
    text = MULTISPACE_PATTERN.sub(" ", text).strip(" ,")
    return text


def format_bullets(text: str) -> str | None:
    candidate = re.sub(
        r"^(?:make|create|write)\s+(?:a\s+)?(?:bullet\s+)?list(?:\s+of)?\s+",
        "",
        text,
        flags=re.IGNORECASE,
    )
    candidate = re.sub(r"^(?:notes?|bullet(?:s)?)\s+", "", candidate, flags=re.IGNORECASE)
    if "," not in candidate and re.search(r"\band\b", candidate, flags=re.IGNORECASE):
        flattened = re.sub(r"\band\b", " ", candidate, flags=re.IGNORECASE)
        items = flattened.split()
    else:
        items = re.split(r",|\band\b", candidate)
    cleaned_items = [sentence_case(item.strip(" .")) for item in items if item.strip()]
    if len(cleaned_items) < 2:
        return None
    return "\n".join(f"- {item}" for item in cleaned_items)


def format_sentence(text: str, *, mode: DictationMode) -> str:
    sentence = sentence_case(text)
    if not sentence:
        return sentence

    if sentence[-1] not in ".!?":
        opener = sentence.split(" ", 1)[0].lower()
        punctuation = "?"
        if mode == DictationMode.CHAT:
            punctuation = "."
        elif opener not in QUESTION_OPENERS:
            punctuation = "."
        sentence = f"{sentence}{punctuation}"
    return sentence


def sentence_case(text: str) -> str:
    compact = MULTISPACE_PATTERN.sub(" ", text).strip()
    if not compact:
        return compact
    return compact[0].upper() + compact[1:]
