from __future__ import annotations

import io
import wave

from fastapi.testclient import TestClient

from app.config import Settings, get_settings
from app.dependencies import build_asr_provider, build_cleanup_provider
from app.main import app
from app.providers.mock import MockAsrProvider, MockCleanupProvider
from app.providers.openai_compatible import (
    OpenAICompatibleAsrProvider,
    OpenAICompatibleCleanupProvider,
)


def make_wav(duration_ms: int = 500) -> bytes:
    frame_rate = 16_000
    sample_count = int(frame_rate * (duration_ms / 1000))
    payload = io.BytesIO()
    with wave.open(payload, "wb") as wav_file:
        wav_file.setnchannels(1)
        wav_file.setsampwidth(2)
        wav_file.setframerate(frame_rate)
        wav_file.writeframes(b"\x00\x00" * sample_count)
    return payload.getvalue()


def client_with_settings(settings: Settings) -> TestClient:
    app.dependency_overrides[get_settings] = lambda: settings
    return TestClient(app)


def teardown_function() -> None:
    app.dependency_overrides.clear()


def test_health_returns_provider_selection() -> None:
    client = client_with_settings(Settings(asr_provider="mock", cleanup_provider="mock"))

    response = client.get("/health")

    assert response.status_code == 200
    assert response.json() == {
        "status": "ok",
        "asr_provider": "mock",
        "cleanup_provider": "mock",
    }


def test_transcribe_returns_expected_shape() -> None:
    client = client_with_settings(Settings(asr_provider="mock", cleanup_provider="mock"))

    response = client.post(
        "/v1/transcribe",
        data={"mode": "notes", "app_name": "Obsidian"},
        files={"file": ("sample.wav", make_wav(750), "audio/wav")},
    )

    body = response.json()
    assert response.status_code == 200
    assert body["raw_text"] == "make a list eggs milk bread and apples"
    assert body["duration_ms"] == 750
    assert body["language"] == "en"
    assert body["provider"] == "mock"


def test_cleanup_formats_bullets() -> None:
    client = client_with_settings(Settings(asr_provider="mock", cleanup_provider="mock"))

    response = client.post(
        "/v1/cleanup",
        json={
            "raw_text": "make a list eggs milk bread and apples",
            "mode": "notes",
            "app_name": "Obsidian",
        },
    )

    assert response.status_code == 200
    assert response.json() == {
        "clean_text": "- Eggs\n- Milk\n- Bread\n- Apples",
        "provider": "mock",
    }


def test_cleanup_removes_fillers_and_punctuates() -> None:
    client = client_with_settings(Settings(asr_provider="mock", cleanup_provider="mock"))

    response = client.post(
        "/v1/cleanup",
        json={"raw_text": "um can you send the update tomorrow", "mode": "default"},
    )

    assert response.status_code == 200
    assert response.json()["clean_text"] == "Can you send the update tomorrow?"


def test_provider_factories_support_shell_adapters() -> None:
    settings = Settings(asr_provider="openai_compatible", cleanup_provider="openai_compatible")

    assert isinstance(build_asr_provider(settings), OpenAICompatibleAsrProvider)
    assert isinstance(build_cleanup_provider(settings), OpenAICompatibleCleanupProvider)
    assert isinstance(build_asr_provider(Settings(asr_provider="mock")), MockAsrProvider)
    assert isinstance(build_cleanup_provider(Settings(cleanup_provider="mock")), MockCleanupProvider)
