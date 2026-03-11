from __future__ import annotations

from contextlib import asynccontextmanager

from fastapi import Depends, FastAPI, File, Form, HTTPException, UploadFile
from fastapi.middleware.cors import CORSMiddleware

from app.audio import estimate_audio_duration_ms
from app.config import Settings, get_settings
from app.dependencies import (
    build_asr_provider,
    build_cleanup_provider,
    get_asr_provider,
    get_cleanup_provider,
)
from app.providers.asr import AsrProvider
from app.providers.cleanup import CleanupProvider
from app.providers.mlx_whisper import (
    MlxWhisperProviderExecutionError,
    MlxWhisperProviderUnavailableError,
)
from app.providers.openai_compatible import ProviderNotConfiguredError
from app.schemas import (
    CleanupRequest,
    CleanupResponse,
    DictationMode,
    HealthResponse,
    TranscriptionResponse,
)

@asynccontextmanager
async def lifespan(_: FastAPI):
    settings = get_settings()
    build_asr_provider(settings)
    build_cleanup_provider(settings)
    yield


app = FastAPI(title="Tator API", version="0.1.0", lifespan=lifespan)
app.add_middleware(
    CORSMiddleware,
    allow_origins=get_settings().cors_allowed_origins,
    allow_credentials=False,
    allow_methods=["*"],
    allow_headers=["*"],
)


@app.get("/health", response_model=HealthResponse)
async def health(settings: Settings = Depends(get_settings)) -> HealthResponse:
    return HealthResponse(
        asr_provider=settings.asr_provider,
        cleanup_provider=settings.cleanup_provider,
    )


@app.post("/v1/transcribe", response_model=TranscriptionResponse)
async def transcribe(
    file: UploadFile = File(...),
    mode: DictationMode = Form(DictationMode.DEFAULT),
    app_name: str | None = Form(default=None),
    provider: AsrProvider = Depends(get_asr_provider),
) -> TranscriptionResponse:
    audio = await file.read()
    duration_ms = estimate_audio_duration_ms(audio)

    try:
        return await provider.transcribe(
            audio=audio,
            filename=file.filename,
            content_type=file.content_type,
            mode=mode,
            app_name=app_name,
            duration_ms=duration_ms,
        )
    except (ProviderNotConfiguredError, MlxWhisperProviderUnavailableError) as exc:
        raise HTTPException(status_code=501, detail=str(exc)) from exc
    except MlxWhisperProviderExecutionError as exc:
        raise HTTPException(status_code=501, detail=str(exc)) from exc


@app.post("/v1/cleanup", response_model=CleanupResponse)
async def cleanup(
    request: CleanupRequest,
    provider: CleanupProvider = Depends(get_cleanup_provider),
) -> CleanupResponse:
    try:
        return await provider.cleanup(
            raw_text=request.raw_text,
            mode=request.mode,
            app_name=request.app_name,
            preserve_raw=request.preserve_raw,
        )
    except ProviderNotConfiguredError as exc:
        raise HTTPException(status_code=501, detail=str(exc)) from exc
