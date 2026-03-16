#!/usr/bin/env bash

set -euo pipefail

repo_root=$(
  cd "$(dirname "${BASH_SOURCE[0]}")/.."
  pwd
)

image_tag=${TAURI_LINUX_DOCKER_IMAGE:-tator-desktop-linux-builder}
docker_platform=${DOCKER_PLATFORM:-linux/amd64}
container_home=/tmp/tator-home

printf 'Building Docker image %s for %s\n' "$image_tag" "$docker_platform"
docker build \
  --platform "$docker_platform" \
  -f "$repo_root/docker/desktop-linux-builder.Dockerfile" \
  -t "$image_tag" \
  "$repo_root"

printf 'Running Linux Tauri build inside Docker\n'
docker run --rm \
  --platform "$docker_platform" \
  --user "$(id -u):$(id -g)" \
  -e HOME="$container_home" \
  -e npm_config_cache="$container_home/.npm" \
  -v "$repo_root:/workspace" \
  -w /workspace \
  "$image_tag" \
  bash -lc "mkdir -p \"$container_home\" && npm ci && env CI=true npm run tauri:build --workspace @tator/desktop"

printf 'Linux bundles are available under %s\n' "$repo_root/apps/desktop/src-tauri/target/release/bundle"
