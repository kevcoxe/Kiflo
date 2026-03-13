from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import struct
import types
import wave

import numpy as np


HELPER_SCRIPT = (
    Path(__file__).resolve().parents[3]
    / "apps"
    / "desktop"
    / "src-tauri"
    / "resources"
    / "python"
    / "tator_mlx_helper.py"
)


def load_helper_module():
    spec = importlib.util.spec_from_file_location("tator_mlx_helper_test", HELPER_SCRIPT)
    assert spec is not None
    assert spec.loader is not None

    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def write_wave_file(path: Path, samples: tuple[int, ...], sample_rate: int = 16_000) -> None:
    payload = struct.pack(f"<{len(samples)}h", *samples)
    with wave.open(str(path), "wb") as wav_file:
        wav_file.setnchannels(1)
        wav_file.setsampwidth(2)
        wav_file.setframerate(sample_rate)
        wav_file.writeframes(payload)


def test_load_waveform_normalizes_pcm_samples(tmp_path) -> None:
    helper = load_helper_module()
    audio_path = tmp_path / "sample.wav"
    write_wave_file(audio_path, (-32768, 0, 32767))

    waveform = helper.load_waveform(str(audio_path))

    assert waveform.dtype == np.float32
    assert np.allclose(waveform, np.array([-1.0, 0.0, 32767 / 32768], dtype=np.float32))


def test_transcribe_audio_forwards_expected_arguments(tmp_path, monkeypatch, capsys) -> None:
    helper = load_helper_module()
    audio_path = tmp_path / "sample.wav"
    model_path = tmp_path / "model"
    model_path.mkdir()
    write_wave_file(audio_path, (0, 8192, -8192, 0))
    captured: dict[str, object] = {}

    def fake_transcribe(waveform, **kwargs):
        captured["waveform"] = waveform
        captured["kwargs"] = kwargs
        return {"text": " Hello from helper ", "language": "en"}

    monkeypatch.setattr(
        helper.importlib,
        "import_module",
        lambda name: types.SimpleNamespace(transcribe=fake_transcribe),
    )

    body = helper.transcribe_audio(str(audio_path), str(model_path), "en")

    assert body == {"text": "Hello from helper", "language": "en"}
    assert isinstance(captured["waveform"], np.ndarray)
    assert captured["kwargs"] == {
        "path_or_hf_repo": str(model_path),
        "language": "en",
        "verbose": False,
        "word_timestamps": False,
        "condition_on_previous_text": False,
    }


def test_download_model_uses_local_dir_without_symlinks(tmp_path, monkeypatch, capsys) -> None:
    helper = load_helper_module()
    destination = tmp_path / "model"
    calls: list[dict[str, object]] = []

    def fake_snapshot_download(**kwargs):
        calls.append(kwargs)
        return str(destination)

    monkeypatch.setattr(
        helper.importlib,
        "import_module",
        lambda name: types.SimpleNamespace(snapshot_download=fake_snapshot_download),
    )

    helper.download_model("mlx-community/whisper-tiny.en-mlx", str(destination))

    body = json.loads(capsys.readouterr().out)
    assert body == {"model_path": str(destination)}
    assert calls == [
        {
            "repo_id": "mlx-community/whisper-tiny.en-mlx",
            "local_dir": str(destination),
            "local_dir_use_symlinks": False,
        }
    ]


def test_serve_processes_line_delimited_requests(tmp_path, monkeypatch, capsys) -> None:
    helper = load_helper_module()
    audio_path = tmp_path / "sample.wav"
    model_path = tmp_path / "model"
    model_path.mkdir()
    write_wave_file(audio_path, (0, 4096, -4096, 0))

    monkeypatch.setattr(
        helper.importlib,
        "import_module",
        lambda name: types.SimpleNamespace(
            transcribe=lambda *_args, **_kwargs: {"text": " hot helper ", "language": "en"}
        ),
    )
    monkeypatch.setattr(
        helper.sys,
        "stdin",
        iter(
            [
                json.dumps(
                    {
                        "command": "transcribe",
                        "audio_path": str(audio_path),
                        "model_path": str(model_path),
                        "language": "en",
                    }
                )
                + "\n"
            ]
        ),
    )

    helper.serve()

    body = json.loads(capsys.readouterr().out.strip())
    assert body == {"text": "hot helper", "language": "en"}


def test_preload_model_warms_model_holder(tmp_path, monkeypatch) -> None:
    helper = load_helper_module()
    model_path = tmp_path / "model"
    model_path.mkdir()
    calls: list[tuple[str, object]] = []

    class FakeModelHolder:
        @staticmethod
        def get_model(path, dtype):
            calls.append((path, dtype))
            return object()

    def fake_import_module(name: str):
        if name == "mlx.core":
            return types.SimpleNamespace(float16="float16")
        if name == "mlx_whisper.transcribe":
            return types.SimpleNamespace(ModelHolder=FakeModelHolder)
        raise AssertionError(f"unexpected import {name}")

    monkeypatch.setattr(helper.importlib, "import_module", fake_import_module)

    body = helper.preload_model(str(model_path))

    assert body == {"status": "ready"}
    assert calls == [(str(model_path), "float16")]
