from __future__ import annotations

import asyncio
import io
import wave

from fastapi.testclient import TestClient
import pytest

from app.config import Settings, get_settings
from app.dependencies import build_asr_provider, build_cleanup_provider
from app.main import app
from app.providers.mlx_whisper import (
    MlxWhisperAsrProvider,
    MlxWhisperProviderExecutionError,
    MlxWhisperProviderUnavailableError,
    verify_mlx_runtime,
)
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
    verify_mlx_runtime.cache_clear()


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
        data={"mode": "bullet_list", "app_name": "Obsidian"},
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
            "mode": "bullet_list",
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


def test_api_allows_tauri_dev_origin() -> None:
    client = client_with_settings(Settings(asr_provider="mock", cleanup_provider="mock"))

    response = client.options(
        "/v1/cleanup",
        headers={
            "Origin": "http://127.0.0.1:1420",
            "Access-Control-Request-Method": "POST",
            "Access-Control-Request-Headers": "content-type",
        },
    )

    assert response.status_code == 200
    assert response.headers["access-control-allow-origin"] == "http://127.0.0.1:1420"


def test_provider_factories_support_shell_adapters() -> None:
    settings = Settings(asr_provider="openai_compatible", cleanup_provider="openai_compatible")

    assert isinstance(build_asr_provider(settings), OpenAICompatibleAsrProvider)
    assert isinstance(build_cleanup_provider(settings), OpenAICompatibleCleanupProvider)
    assert isinstance(build_asr_provider(Settings(asr_provider="mock")), MockAsrProvider)
    assert isinstance(build_cleanup_provider(Settings(cleanup_provider="mock")), MockCleanupProvider)


def test_provider_factory_supports_mlx_whisper(monkeypatch) -> None:
    monkeypatch.setattr(
        MlxWhisperAsrProvider,
        "validate_environment",
        lambda self: None,
    )

    provider = build_asr_provider(Settings(asr_provider="mlx_whisper"))

    assert isinstance(provider, MlxWhisperAsrProvider)
    assert provider.model_repo == "mlx-community/whisper-tiny.en-mlx"
    assert provider.language == "en"


def test_mlx_provider_returns_existing_response_shape(monkeypatch) -> None:
    monkeypatch.setattr(
        MlxWhisperAsrProvider,
        "validate_environment",
        lambda self: None,
    )
    monkeypatch.setattr(
        MlxWhisperAsrProvider,
        "resolve_model_path",
        lambda self: "/tmp/mlx-model",
    )

    captured: dict[str, object] = {}

    def fake_transcribe(audio_path: str, **kwargs):
        captured["audio_path"] = audio_path
        captured["kwargs"] = kwargs
        return {
            "text": " Hello from MLX ",
            "language": "en",
        }

    monkeypatch.setattr(
        MlxWhisperAsrProvider,
        "get_transcribe_callable",
        staticmethod(lambda: fake_transcribe),
    )

    provider = MlxWhisperAsrProvider(
        model_repo="mlx-community/whisper-tiny.en-mlx",
        language="en",
    )

    response = run_async(
        provider.transcribe(
            audio=make_wav(250),
            filename="sample.wav",
            content_type="audio/wav",
            mode="default",
            app_name="TextEdit",
            duration_ms=250,
        )
    )

    assert response == {
        "raw_text": "Hello from MLX",
        "language": "en",
        "duration_ms": 250,
        "provider": "mlx_whisper",
    }
    assert captured["kwargs"] == {
        "path_or_hf_repo": "/tmp/mlx-model",
        "language": "en",
        "verbose": False,
        "word_timestamps": False,
        "condition_on_previous_text": False,
    }


def test_mlx_provider_uses_download_root_when_configured(monkeypatch) -> None:
    monkeypatch.setattr(
        MlxWhisperAsrProvider,
        "validate_environment",
        lambda self: None,
    )
    calls: list[tuple[str, str]] = []

    def fake_snapshot_download(*, repo_id: str, cache_dir: str):
        calls.append((repo_id, cache_dir))
        return "/tmp/resolved-model"

    monkeypatch.setattr(
        MlxWhisperAsrProvider,
        "get_snapshot_download_callable",
        staticmethod(lambda: fake_snapshot_download),
    )

    provider = MlxWhisperAsrProvider(
        model_repo="mlx-community/whisper-tiny.en-mlx",
        language="en",
        download_root="/tmp/mlx-cache",
    )

    assert provider.resolve_model_path() == "/tmp/resolved-model"
    assert calls == [("mlx-community/whisper-tiny.en-mlx", "/tmp/mlx-cache")]


def test_mlx_provider_rejects_unsupported_platform(monkeypatch) -> None:
    monkeypatch.setattr("app.providers.mlx_whisper.platform.system", lambda: "Linux")
    monkeypatch.setattr("app.providers.mlx_whisper.platform.machine", lambda: "x86_64")

    with pytest.raises(MlxWhisperProviderUnavailableError):
        MlxWhisperAsrProvider(
            model_repo="mlx-community/whisper-tiny.en-mlx",
            language="en",
        )


def test_mlx_provider_surfaces_transcription_failures(monkeypatch) -> None:
    monkeypatch.setattr(
        MlxWhisperAsrProvider,
        "validate_environment",
        lambda self: None,
    )
    monkeypatch.setattr(
        MlxWhisperAsrProvider,
        "get_transcribe_callable",
        staticmethod(lambda: (_ for _ in ()).throw(RuntimeError("mlx failed"))),
    )

    provider = MlxWhisperAsrProvider(
        model_repo="mlx-community/whisper-tiny.en-mlx",
        language="en",
    )

    with pytest.raises(MlxWhisperProviderExecutionError):
        run_async(
            provider.transcribe(
                audio=make_wav(250),
                filename="sample.wav",
                content_type="audio/wav",
                mode="default",
                app_name=None,
                duration_ms=250,
            )
        )


def test_mlx_provider_rejects_broken_runtime_import(monkeypatch) -> None:
    monkeypatch.setattr("app.providers.mlx_whisper.platform.system", lambda: "Darwin")
    monkeypatch.setattr("app.providers.mlx_whisper.platform.machine", lambda: "arm64")
    monkeypatch.setattr("app.providers.mlx_whisper.shutil.which", lambda _: "/opt/homebrew/bin/ffmpeg")
    monkeypatch.setattr("app.providers.mlx_whisper.importlib.util.find_spec", lambda _: object())

    class Result:
        returncode = -6
        stdout = ""
        stderr = (
            "*** Terminating app due to uncaught exception 'NSRangeException'\n"
            "libc++abi: terminating due to uncaught exception of type NSException"
        )

    monkeypatch.setattr("app.providers.mlx_whisper.subprocess.run", lambda *args, **kwargs: Result())

    with pytest.raises(MlxWhisperProviderUnavailableError) as exc_info:
        MlxWhisperAsrProvider(
            model_repo="mlx-community/whisper-tiny.en-mlx",
            language="en",
        )

    message = str(exc_info.value)
    assert "could not initialize MLX" in message
    assert "Python 3.12" in message


def test_startup_fails_fast_for_invalid_mlx_environment(monkeypatch) -> None:
    monkeypatch.setattr("app.main.get_settings", lambda: Settings(asr_provider="mlx_whisper"))
    monkeypatch.setattr(
        MlxWhisperAsrProvider,
        "validate_environment",
        lambda self: (_ for _ in ()).throw(MlxWhisperProviderUnavailableError("mlx unavailable")),
    )

    with pytest.raises(MlxWhisperProviderUnavailableError):
        with TestClient(app):
            pass


def run_async(awaitable):
    return asyncio.run(awaitable).model_dump()
