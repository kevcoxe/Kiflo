import argparse
import warnings

warnings.filterwarnings(
    "ignore",
    message="'audioop' is deprecated and slated for removal in Python 3.13",
    category=DeprecationWarning,
)

import audioop
import importlib
import json
from pathlib import Path
import sys
from typing import Any
import wave

import numpy as np

TARGET_SAMPLE_RATE = 16_000


def load_waveform(audio_path: str) -> np.ndarray:
    with wave.open(audio_path, "rb") as wav_file:
        channels = wav_file.getnchannels()
        sample_width = wav_file.getsampwidth()
        sample_rate = wav_file.getframerate()
        frames = wav_file.readframes(wav_file.getnframes())

    if channels != 1:
        raise RuntimeError(
            f"Expected mono WAV input but received {channels} channels."
        )

    if sample_width not in (1, 2, 4):
        raise RuntimeError(
            f"Unsupported WAV sample width: {sample_width * 8} bits per sample."
        )

    if sample_width != 2:
        frames = audioop.lin2lin(frames, sample_width, 2)

    if sample_rate != TARGET_SAMPLE_RATE:
        frames, _ = audioop.ratecv(
            frames, 2, 1, sample_rate, TARGET_SAMPLE_RATE, None
        )

    return np.frombuffer(frames, dtype=np.int16).astype(np.float32) / 32768.0


def transcribe_audio(audio_path: str, model_path: str, language: str) -> dict[str, str]:
    audio_file = Path(audio_path)
    local_model = Path(model_path)
    if not audio_file.exists():
        raise RuntimeError(f"Audio file does not exist: {audio_file}")
    if not local_model.exists():
        raise RuntimeError(f"Model is not installed at: {local_model}")

    mlx_whisper = importlib.import_module("mlx_whisper")
    waveform = load_waveform(str(audio_file))
    result = mlx_whisper.transcribe(
        waveform,
        path_or_hf_repo=str(local_model),
        language=language,
        verbose=False,
        word_timestamps=False,
        condition_on_previous_text=False,
    )
    return {
        "text": result.get("text", "").strip(),
        "language": result.get("language", language),
    }


def preload_model(model_path: str) -> dict[str, str]:
    local_model = Path(model_path)
    if not local_model.exists():
        raise RuntimeError(f"Model is not installed at: {local_model}")

    mx = importlib.import_module("mlx.core")
    transcribe_module = importlib.import_module("mlx_whisper.transcribe")
    transcribe_module.ModelHolder.get_model(str(local_model), mx.float16)
    return {"status": "ready"}


def print_json(payload: dict[str, Any]) -> None:
    print(json.dumps(payload), flush=True)


def serve() -> None:
    for line in sys.stdin:
        request = line.strip()
        if not request:
            continue

        try:
            payload = json.loads(request)
            command = payload.get("command")

            if command == "transcribe":
                print_json(
                    transcribe_audio(
                        payload["audio_path"],
                        payload["model_path"],
                        payload.get("language", "en"),
                    )
                )
            elif command == "preload-model":
                print_json(preload_model(payload["model_path"]))
            elif command == "download-model":
                download_model(payload["model_repo"], payload["model_path"])
            else:
                raise RuntimeError(f"Unsupported helper command: {command}")
        except Exception as exc:  # noqa: BLE001
            print_json({"error": str(exc)})


def download_model(model_repo: str, model_path: str) -> None:
    destination = Path(model_path)
    destination.parent.mkdir(parents=True, exist_ok=True)
    huggingface_hub = importlib.import_module("huggingface_hub")
    downloaded = huggingface_hub.snapshot_download(
        repo_id=model_repo,
        local_dir=str(destination),
        local_dir_use_symlinks=False,
    )
    print_json({"model_path": str(downloaded)})


def main() -> int:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)

    transcribe = subparsers.add_parser("transcribe")
    transcribe.add_argument("--audio-path", required=True)
    transcribe.add_argument("--model-path", required=True)
    transcribe.add_argument("--language", default="en")

    preload = subparsers.add_parser("preload-model")
    preload.add_argument("--model-path", required=True)

    download = subparsers.add_parser("download-model")
    download.add_argument("--model-repo", required=True)
    download.add_argument("--model-path", required=True)

    subparsers.add_parser("serve")

    args = parser.parse_args()
    try:
        if args.command == "transcribe":
            print_json(transcribe_audio(args.audio_path, args.model_path, args.language))
        elif args.command == "preload-model":
            print_json(preload_model(args.model_path))
        elif args.command == "serve":
            serve()
        else:
            download_model(args.model_repo, args.model_path)
    except Exception as exc:  # noqa: BLE001
        print(str(exc), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
