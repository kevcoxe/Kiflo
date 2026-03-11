# Tator

Tator is a macOS-first dictation MVP built as a Tauri desktop app with a FastAPI backend. The repository is organized as a small monorepo:

- `apps/desktop`: Tauri v2 + React/TypeScript desktop app.
- `apps/api`: FastAPI service with pluggable ASR and cleanup providers.
- `packages/shared-types`: shared TypeScript contracts used by the desktop app.

## Prerequisites

- Node.js 23+
- npm 10+
- Python 3.12+
- `uv`
- Rust toolchain with Cargo installed
- Xcode command line tools

For Apple-Silicon local ASR with MLX:

- Apple Silicon Mac
- Python 3.12 is the safest runtime for the API environment right now
- `ffmpeg` in `PATH` via `brew install ffmpeg`

## Bootstrap

1. Install JavaScript dependencies with `npm install`.
2. Install API dependencies with `uv sync --project apps/api --extra dev`.
3. Copy `.env.example`, `apps/api/.env.example`, and `apps/desktop/.env.example` if you need local overrides.
4. Start the API with `npm run dev:api`.
5. Start the desktop UI with `npm run tauri:dev`.

To enable local MLX Whisper transcription in the API:

1. Set `TATOR_ASR_PROVIDER=mlx_whisper` in `apps/api/.env`.
2. Optionally override `TATOR_MLX_WHISPER_MODEL_REPO` and `TATOR_MLX_WHISPER_DOWNLOAD_ROOT`.
3. The first transcription will download the configured MLX model if it is not already cached.
4. If MLX fails to initialize, recreate `apps/api/.venv` with Python 3.12 and resync dependencies.

## Verification

- `npm test` runs the desktop Vitest suite, the FastAPI pytest suite, and the Rust unit tests.
- `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml` validates the native Tauri layer.

## Current MVP Slice

- Toggle global hotkey (`Option+Space`)
- Local audio capture
- Mock cleanup pipeline
- Optional Apple-Silicon local ASR via MLX Whisper
- Clipboard-based paste and restore flow
- Local SQLite-backed settings and history
- Fallback popup when insertion fails
