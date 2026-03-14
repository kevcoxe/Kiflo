#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: ./scripts/debug-desktop-release-local.sh v0.2.0 [--ref HEAD] [--output-dir /tmp/out] [--allow-dirty] [--keep-worktree]

Builds the desktop app in a temporary git worktree using the same high-level steps as the
GitHub release workflow, archives the built .app, and writes diagnostics next to it.
EOF
}

fail() {
  printf '%s\n' "$1" >&2
  exit 1
}

abs_path() {
  local target=$1

  if [[ -d "$target" ]]; then
    (
      cd "$target"
      pwd
    )
    return
  fi

  (
    cd "$(dirname "$target")"
    printf '%s/%s\n' "$(pwd)" "$(basename "$target")"
  )
}

repo_root=$(
  cd "$(dirname "${BASH_SOURCE[0]}")/.."
  pwd
)

tag_name=""
git_ref="HEAD"
output_dir=""
allow_dirty=0
keep_worktree=0
worktree_dir=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --ref)
      git_ref=${2:-}
      shift 2
      ;;
    --output-dir)
      output_dir=${2:-}
      shift 2
      ;;
    --allow-dirty)
      allow_dirty=1
      shift
      ;;
    --keep-worktree)
      keep_worktree=1
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    -*)
      fail "Unknown option: $1"
      ;;
    *)
      if [[ -n "$tag_name" ]]; then
        fail "Only one release version may be provided."
      fi
      tag_name=$1
      shift
      ;;
  esac
done

if [[ -z "$tag_name" ]]; then
  usage
  exit 1
fi

if [[ $allow_dirty -eq 0 ]] && [[ -n "$(git -C "$repo_root" status --porcelain)" ]]; then
  fail "Working tree is dirty. Use a clean checkout or rerun with --allow-dirty if you accept that the temp worktree will only include committed changes."
fi

if [[ -z "$output_dir" ]]; then
  output_dir="$(mktemp -d "${TMPDIR:-/tmp}/tator-release-debug-output.XXXXXX")"
else
  mkdir -p "$output_dir"
  output_dir=$(abs_path "$output_dir")
fi

cleanup() {
  local status=$?

  if [[ -n "$worktree_dir" ]]; then
    if [[ $status -ne 0 ]]; then
      printf 'Build failed. Preserving worktree at %s for inspection.\n' "$worktree_dir" >&2
      return
    fi

    if [[ $keep_worktree -eq 0 ]]; then
      git -C "$repo_root" worktree remove --force "$worktree_dir" >/dev/null 2>&1 || true
    else
      printf 'Kept worktree at %s\n' "$worktree_dir"
    fi
  fi
}

trap cleanup EXIT

worktree_dir="$(mktemp -d "${TMPDIR:-/tmp}/tator-release-debug-worktree.XXXXXX")"
git -C "$repo_root" worktree add --detach "$worktree_dir" "$git_ref" >/dev/null

run_in_worktree() {
  (
    cd "$worktree_dir"
    "$@"
  )
}

printf 'Building release parity bundle in %s\n' "$worktree_dir"

run_in_worktree env CI=true npm ci
run_in_worktree env CI=true uv sync --project apps/mlx-runtime --extra dev
run_in_worktree node ./scripts/sync-desktop-version.mjs "$tag_name"
run_in_worktree env CI=true npm run tauri:build --workspace @tator/desktop

bundle_dir="$worktree_dir/apps/desktop/src-tauri/target/release/bundle/macos"
if [[ ! -d "$bundle_dir" ]]; then
  fail "Could not find the macOS bundle directory at $bundle_dir."
fi

app_path=$(find "$bundle_dir" -maxdepth 1 -name '*.app' -print | sort | head -n 1)
if [[ -z "$app_path" ]] || [[ ! -d "$app_path" ]]; then
  fail "Could not find the built .app bundle under apps/desktop/src-tauri/target/release/bundle/macos."
fi

archive_name="$(basename "$app_path" .app)-${tag_name#v}-local.app.tar.gz"
archive_path="$output_dir/$archive_name"
tar -C "$(dirname "$app_path")" -czf "$archive_path" "$(basename "$app_path")"

"$worktree_dir/scripts/inspect-desktop-bundle.sh" "$app_path" "$output_dir/diagnostics"

{
  printf 'tag_name=%s\n' "$tag_name"
  printf 'git_ref=%s\n' "$git_ref"
  printf 'git_sha=%s\n' "$(git -C "$worktree_dir" rev-parse HEAD)"
  printf 'worktree_dir=%s\n' "$worktree_dir"
  printf 'app_path=%s\n' "$app_path"
  printf 'archive_path=%s\n' "$archive_path"
  printf 'diagnostics_dir=%s\n' "$output_dir/diagnostics"
} >"$output_dir/metadata.txt"

printf 'Built archive: %s\n' "$archive_path"
printf 'Diagnostics: %s\n' "$output_dir/diagnostics"
