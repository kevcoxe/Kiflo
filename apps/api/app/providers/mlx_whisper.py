from __future__ import annotations

import importlib
import importlib.util
import platform
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path

from app.providers.asr import AsrProvider
from app.schemas import DictationMode, TranscriptionResponse


class MlxWhisperProviderUnavailableError(RuntimeError):
    """Raised when local MLX Whisper cannot run in the current environment."""


class MlxWhisperProviderExecutionError(RuntimeError):
    """Raised when the MLX Whisper inference call fails."""


@lru_cache(maxsize=1)
def verify_mlx_runtime(python_executable: str) -> None:
    result = subprocess.run(
        [python_executable, "-c", "import mlx_whisper"],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode == 0:
        return

    output = "\n".join(
        part.strip() for part in (result.stderr, result.stdout) if part and part.strip()
    )
    if "NSRangeException" in output or "terminating due to uncaught exception" in output:
        detail = "The MLX runtime aborted during import."
    else:
        lines = [line.strip() for line in output.splitlines() if line.strip()]
        detail = lines[-1] if lines else f"Import check exited with status {result.returncode}."

    raise MlxWhisperProviderUnavailableError(
        "The mlx_whisper ASR provider could not initialize MLX in the current "
        f"Python runtime ({platform.python_version()}). {detail} Recreate the API "
        "environment with Python 3.12, then run `uv sync --project apps/api --extra dev`."
    )


@dataclass(slots=True)
class MlxWhisperAsrProvider(AsrProvider):
    model_repo: str
    language: str
    download_root: str | None = None

    name: str = "mlx_whisper"

    def __post_init__(self) -> None:
        self.validate_environment()

    def validate_environment(self) -> None:
        if platform.system() != "Darwin" or platform.machine() != "arm64":
            raise MlxWhisperProviderUnavailableError(
                "The mlx_whisper ASR provider requires an Apple Silicon Mac."
            )

        if shutil.which("ffmpeg") is None:
            raise MlxWhisperProviderUnavailableError(
                "The mlx_whisper ASR provider requires ffmpeg in PATH. "
                "Install it with `brew install ffmpeg`."
            )

        if importlib.util.find_spec("mlx_whisper") is None:
            raise MlxWhisperProviderUnavailableError(
                "The mlx_whisper package is not installed in the API environment."
            )

        verify_mlx_runtime(sys.executable)

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
        del content_type, mode, app_name

        suffix = Path(filename or "dictation.wav").suffix or ".wav"
        with tempfile.NamedTemporaryFile(delete=False, suffix=suffix) as handle:
            handle.write(audio)
            temp_path = Path(handle.name)

        try:
            model_path = self.resolve_model_path()
            transcribe = self.get_transcribe_callable()
            result = transcribe(
                str(temp_path),
                path_or_hf_repo=model_path,
                language=self.language,
                verbose=False,
                word_timestamps=False,
                condition_on_previous_text=False,
            )
        except Exception as exc:
            raise MlxWhisperProviderExecutionError(
                f"mlx_whisper transcription failed: {exc}"
            ) from exc
        finally:
            temp_path.unlink(missing_ok=True)

        return TranscriptionResponse(
            raw_text=result["text"].strip(),
            language=result.get("language", self.language),
            duration_ms=duration_ms,
            provider=self.name,
        )

    def resolve_model_path(self) -> str:
        if Path(self.model_repo).exists():
            return self.model_repo

        if self.download_root is None:
            return self.model_repo

        snapshot_download = self.get_snapshot_download_callable()
        downloaded = snapshot_download(
            repo_id=self.model_repo,
            cache_dir=self.download_root,
        )
        return str(downloaded)

    @staticmethod
    def get_transcribe_callable():
        module = importlib.import_module("mlx_whisper")
        return module.transcribe

    @staticmethod
    def get_snapshot_download_callable():
        module = importlib.import_module("huggingface_hub")
        return module.snapshot_download
