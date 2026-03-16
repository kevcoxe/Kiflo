# Tator

Tator is a desktop dictation app built as a Tauri app with local transcription and cleanup inside the app. The repository is organized as a small monorepo:

- `apps/desktop`: Tauri v2 + React/TypeScript desktop app.
- `apps/mlx-runtime`: legacy Python MLX prototype kept for reference while the shipped app uses a native backend.
- `packages/shared-types`: shared TypeScript contracts used by the desktop app.

## Prerequisites

- Node.js 23+
- npm 10+
- Rust toolchain with Cargo installed
- CMake
- Xcode command line tools on macOS
- Visual Studio C++ build tools on Windows

## Bootstrap

1. Install JavaScript dependencies with `npm install`.
2. Start the desktop UI with `npm run tauri:dev`.

The app now uses a bundled native `whisper.cpp` backend through Rust bindings. `tauri dev` and `tauri build` automatically run `npm run prepare:whisper-assets`, which downloads the official `ggml-base.en.bin` model and bundles it into the desktop app. Users can optionally download the larger `ggml-small.en.bin` model from Settings for higher transcription quality.

Model downloads try multiple sources in order: Hugging Face first, then the repo's GitHub Releases assets as a fallback. If you need to point builds or app downloads at your own mirror, set `TATOR_WHISPER_MODEL_MIRROR_BASE_URL` to a directory that serves the `ggml-*.bin` files.

The shipped desktop app does not talk to a separate HTTP API. Transcription and cleanup run through native Tauri commands.

## Verification

- `npm test` runs the desktop Vitest suite and the Rust unit tests.
- `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml` validates the native Tauri layer.

## GitHub Release Workflow

The repository includes a GitHub Actions workflow at `.github/workflows/release-desktop.yml`
that builds the desktop app for macOS, Windows, and Linux and publishes the generated artifacts to the
repo's GitHub Releases page.

To cut a release:

1. Run `npm run release:prep -- v0.3.0` to draft the next desktop release entry in `CHANGELOG.md`.
2. Review and polish `CHANGELOG.md`.
3. Commit the changelog and any related release changes.
4. Push a version tag that starts with `v`, for example `v0.3.0`.
5. GitHub Actions will sync the desktop app version from that tag, read the matching section from
   `CHANGELOG.md`, and attach the macOS, Windows, and Linux Tauri bundles plus the mirrored whisper model files to a GitHub Release for that tag.

The workflow publishes unsigned macOS and Windows artifacts by default. If you need signed or
notarized releases, add the relevant platform credentials as GitHub Actions secrets and extend the
workflow.

Tauri's current distribution docs still require macOS bundles to be created on a Mac, and Windows
cross-compilation from non-Windows hosts is documented as a caveated last resort. Linux is the
platform Tauri explicitly recommends building with Docker or GitHub Actions for reproducibility.

## Debugging GitHub-Built Bundles

- `npm run debug:desktop:release:local -- v0.3.0` builds the desktop app in a temporary clean
  worktree using the same high-level steps as the release workflow, then writes a tarred `.app`
  bundle plus metadata to a local output directory.
- The manual GitHub Actions workflow at `.github/workflows/debug-desktop-bundle.yml` builds debug
  artifacts on `ubuntu-22.04`, `macos-15`, and `windows-2025`, then uploads the full Tauri bundle output plus the
  bundled model manifest so you can compare packaged builds across platforms.

## Local Build Commands

- `npm run build:desktop:macos` builds the macOS app bundle on a Mac host.
- `npm run build:desktop:linux` builds Linux bundles when run on a Linux host with the required Tauri system packages installed.
- `npm run build:desktop:linux:docker` builds Linux bundles in Docker using `docker/desktop-linux-builder.Dockerfile`. By default this uses `linux/amd64` so you can produce x64 Linux artifacts from Apple Silicon Macs too, though emulation will be slower.

The Docker workflow writes Linux artifacts to `apps/desktop/src-tauri/target/release/bundle`.

## Current MVP Slice

- Toggle global hotkey (`Option+Space`)
- Local audio capture
- Native cleanup pipeline inside the desktop app
- Fully bundled local ASR via `whisper.cpp`
- Clipboard-based paste and restore flow
- Local SQLite-backed settings and history
- Fallback popup when insertion fails
