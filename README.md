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

## Bootstrap

1. Install JavaScript dependencies with `npm install`.
2. Install API dependencies with `uv sync --project apps/api --extra dev`.
3. Copy `.env.example`, `apps/api/.env.example`, and `apps/desktop/.env.example` if you need local overrides.
4. Start the API with `npm run dev:api`.
5. Start the desktop UI with `npm run tauri:dev`.

## Verification

- `npm test` runs the desktop Vitest suite, the FastAPI pytest suite, and the Rust unit tests.
- `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml` validates the native Tauri layer.

## Current MVP Slice

- Toggle global hotkey (`Option+Space`)
- Local audio capture
- Mock ASR and cleanup pipeline
- Clipboard-based paste and restore flow
- Local SQLite-backed settings and history
- Fallback popup when insertion fails
