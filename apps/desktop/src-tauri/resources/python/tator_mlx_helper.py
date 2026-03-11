import argparse
import importlib
import json
from pathlib import Path
import sys


def transcribe_audio(audio_path: str, model_path: str, language: str) -> None:
    audio_file = Path(audio_path)
    local_model = Path(model_path)
    if not audio_file.exists():
        raise RuntimeError(f"Audio file does not exist: {audio_file}")
    if not local_model.exists():
        raise RuntimeError(f"Model is not installed at: {local_model}")

    mlx_whisper = importlib.import_module("mlx_whisper")
    result = mlx_whisper.transcribe(
        str(audio_file),
        path_or_hf_repo=str(local_model),
        language=language,
        verbose=False,
        word_timestamps=False,
        condition_on_previous_text=False,
    )
    print(
        json.dumps(
            {
                "text": result.get("text", "").strip(),
                "language": result.get("language", language),
            }
        )
    )


def download_model(model_repo: str, model_path: str) -> None:
    destination = Path(model_path)
    destination.parent.mkdir(parents=True, exist_ok=True)
    huggingface_hub = importlib.import_module("huggingface_hub")
    downloaded = huggingface_hub.snapshot_download(
        repo_id=model_repo,
        local_dir=str(destination),
        local_dir_use_symlinks=False,
    )
    print(json.dumps({"model_path": str(downloaded)}))


def main() -> int:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)

    transcribe = subparsers.add_parser("transcribe")
    transcribe.add_argument("--audio-path", required=True)
    transcribe.add_argument("--model-path", required=True)
    transcribe.add_argument("--language", default="en")

    download = subparsers.add_parser("download-model")
    download.add_argument("--model-repo", required=True)
    download.add_argument("--model-path", required=True)

    args = parser.parse_args()
    try:
        if args.command == "transcribe":
            transcribe_audio(args.audio_path, args.model_path, args.language)
        else:
            download_model(args.model_repo, args.model_path)
    except FileNotFoundError as exc:
        if exc.filename == "ffmpeg":
            print(
                "ffmpeg is required for local transcription but was not found in the app runtime PATH.",
                file=sys.stderr,
            )
        else:
            print(str(exc), file=sys.stderr)
        return 1
    except Exception as exc:  # noqa: BLE001
        print(str(exc), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
