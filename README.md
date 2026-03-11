# Tator

Tator is a macOS-first dictation app built as a Tauri desktop app with local transcription and cleanup inside the app. The repository is organized as a small monorepo:

- `apps/desktop`: Tauri v2 + React/TypeScript desktop app.
- `apps/api`: Python MLX helper/runtime used for development and model tooling.
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
- Python 3.12 is the safest runtime for the helper environment right now
- `ffmpeg` in `PATH` via `brew install ffmpeg`

## Bootstrap

1. Install JavaScript dependencies with `npm install`.
2. Start the desktop UI with `npm run tauri:dev`.

The shipped desktop app no longer talks to a separate HTTP API. Transcription and cleanup run through native Tauri commands.

To enable local MLX Whisper transcription during development:

1. Install the helper runtime with `uv sync --project apps/api --extra dev`.
2. Launch the desktop app with `TATOR_INTERNAL_ASR_PROVIDER=mlx_whisper npm run tauri:dev`.
3. The MLX Whisper tiny English model is bundled into the app and is copied into app data on first use. No network download is required for the model.
4. If the helper runtime is not auto-detected, set `TATOR_PYTHON_EXECUTABLE` to the Python interpreter that has `mlx_whisper` installed.

## Verification

- `npm test` runs the desktop Vitest suite, the helper pytest suite, and the Rust unit tests.
- `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml` validates the native Tauri layer.

## GitHub Release Workflow

The repository now includes a GitHub Actions workflow at `.github/workflows/release-desktop.yml`
that builds the macOS Tauri app and publishes the generated artifacts to the repo's
GitHub Releases page.

To cut a release:

1. Keep the app version aligned in `apps/desktop/package.json`, `apps/desktop/src-tauri/Cargo.toml`,
   and `apps/desktop/src-tauri/tauri.conf.json`.
2. Push a version tag that starts with `v`, for example `v0.1.0`.
3. GitHub Actions will build the desktop app on `macos-latest` and attach the Tauri bundles
   to a GitHub Release for that tag.

The workflow publishes unsigned macOS artifacts by default. If you need a signed or notarized
release, add the relevant Apple credentials as GitHub Actions secrets and extend the workflow.

## Current MVP Slice

- Toggle global hotkey (`Option+Space`)
- Local audio capture
- Native cleanup pipeline inside the desktop app
- Optional Apple-Silicon local ASR via MLX Whisper
- Clipboard-based paste and restore flow
- Local SQLite-backed settings and history
- Fallback popup when insertion fails
