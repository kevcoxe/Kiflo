#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: ./scripts/inspect-desktop-bundle.sh /path/to/Tator.app [/path/to/output-dir]
   or: ./scripts/inspect-desktop-bundle.sh /path/to/runtime [/path/to/output-dir]

Writes bundle diagnostics and packaged-runtime smoke test results to the output directory.
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

capture_cmd() {
  local slug=$1
  shift

  local stdout_file="$output_dir/$slug.stdout.txt"
  local stderr_file="$output_dir/$slug.stderr.txt"
  local exit_file="$output_dir/$slug.exit.txt"
  local command_file="$output_dir/$slug.command.txt"

  mkdir -p "$(dirname "$stdout_file")"
  printf '%q ' "$@" >"$command_file"
  printf '\n' >>"$command_file"

  exec 3>&2
  exec 2>>"$stderr_file"
  set +e
  "$@" >"$stdout_file"
  local status=$?
  set -e
  exec 2>&3
  exec 3>&-

  printf '%s\n' "$status" >"$exit_file"
}

capture_text() {
  local relative_path=$1
  local content=$2

  mkdir -p "$(dirname "$output_dir/$relative_path")"
  printf '%s\n' "$content" >"$output_dir/$relative_path"
}

pick_python_executable() {
  local runtime_root=$1

  local candidates=(
    "$runtime_root/python-home/bin/python3.12"
    "$runtime_root/python-home/bin/python3"
    "$runtime_root/python-home/bin/python"
  )

  local candidate
  for candidate in "${candidates[@]}"; do
    if [[ -f "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done

  return 1
}

run_python_probe() {
  local name=$1
  local code=$2

  capture_cmd "smoke-tests/$name" \
    env \
    PYTHONHOME="$python_home" \
    PYTHONPATH="$site_packages" \
    PYTHONDONTWRITEBYTECODE=1 \
    PYTHONNOUSERSITE=1 \
    "$python_executable" \
    -c \
    "$code"
}

input_path=${1:-}
output_dir=${2:-}

if [[ -z "$input_path" ]] || [[ "$input_path" == "--help" ]] || [[ "$input_path" == "-h" ]]; then
  usage
  exit 0
fi

if [[ ! -e "$input_path" ]]; then
  fail "Input path does not exist: $input_path"
fi

input_path=$(abs_path "$input_path")

if [[ -z "$output_dir" ]]; then
  output_dir="$(mktemp -d "${TMPDIR:-/tmp}/tator-bundle-diagnostics.XXXXXX")"
else
  mkdir -p "$output_dir"
  output_dir=$(abs_path "$output_dir")
fi

app_path=""
runtime_root=""

if [[ -d "$input_path" && "$input_path" == *.app ]]; then
  app_path=$input_path
  runtime_root="$app_path/Contents/Resources/runtime"
elif [[ -d "$input_path" ]]; then
  runtime_root=$input_path
else
  fail "Expected an app bundle directory or a bundled runtime directory."
fi

mkdir -p "$output_dir"

app_binary=""
if [[ -n "$app_path" && -d "$app_path/Contents/MacOS" ]]; then
  app_binary=$(find "$app_path/Contents/MacOS" -maxdepth 1 -type f -print | sort | head -n 1)
fi

manifest_path="$runtime_root/manifest.json"
python_home="$runtime_root/python-home"
site_packages="$runtime_root/site-packages"
python_executable=""
if python_executable=$(pick_python_executable "$runtime_root" 2>/dev/null); then
  :
else
  python_executable=""
fi

libpython_path=""
if [[ -d "$python_home/lib" ]]; then
  libpython_path=$(find "$python_home/lib" -maxdepth 1 -type f -name 'libpython*.dylib' -print | sort | head -n 1 || true)
fi

{
  printf 'input_path=%s\n' "$input_path"
  printf 'output_dir=%s\n' "$output_dir"
  printf 'app_path=%s\n' "${app_path:-<none>}"
  printf 'runtime_root=%s\n' "$runtime_root"
  printf 'app_binary=%s\n' "${app_binary:-<none>}"
  printf 'python_executable=%s\n' "${python_executable:-<none>}"
  printf 'libpython_path=%s\n' "${libpython_path:-<none>}"
  printf 'manifest_path=%s\n' "${manifest_path:-<missing>}"
} >"$output_dir/summary.txt"

capture_cmd "environment/sw_vers" sw_vers
capture_cmd "environment/uname_machine" uname -m
capture_cmd "environment/uname_all" uname -a
capture_cmd "environment/node_version" node --version
capture_cmd "environment/npm_version" npm --version
capture_cmd "environment/uv_version" uv --version
capture_cmd "environment/rustc_version" rustc --version

if [[ -n "$app_path" ]]; then
  capture_cmd "app/xattr_recursive" xattr -lr "$app_path"
  capture_cmd "app/codesign_display" codesign -dvv "$app_path"
  capture_cmd "app/codesign_verify" codesign --verify --deep --strict --verbose=4 "$app_path"
  capture_cmd "app/spctl" spctl -a -vv "$app_path"
fi

if [[ -d "$runtime_root" ]]; then
  capture_cmd "runtime/xattr_recursive" xattr -lr "$runtime_root"
fi

if [[ -f "$manifest_path" ]]; then
  capture_cmd "runtime/manifest" cat "$manifest_path"
else
  capture_text "runtime/manifest.txt" "Missing manifest.json at $manifest_path"
fi

if [[ -n "$app_binary" && -f "$app_binary" ]]; then
  capture_cmd "binaries/app_binary_file" file "$app_binary"
  capture_cmd "binaries/app_binary_lipo" lipo -archs "$app_binary"
  capture_cmd "binaries/app_binary_otool" otool -L "$app_binary"
fi

if [[ -n "$python_executable" && -f "$python_executable" ]]; then
  capture_cmd "binaries/python_file" file "$python_executable"
  capture_cmd "binaries/python_lipo" lipo -archs "$python_executable"
  capture_cmd "binaries/python_otool" otool -L "$python_executable"
  capture_cmd "binaries/python_codesign_display" codesign -dvv "$python_executable"
  capture_cmd "binaries/python_codesign_verify" codesign --verify --verbose=4 "$python_executable"
fi

if [[ -n "$libpython_path" && -f "$libpython_path" ]]; then
  capture_cmd "binaries/libpython_file" file "$libpython_path"
  capture_cmd "binaries/libpython_lipo" lipo -archs "$libpython_path"
  capture_cmd "binaries/libpython_otool" otool -L "$libpython_path"
  capture_cmd "binaries/libpython_codesign_display" codesign -dvv "$libpython_path"
  capture_cmd "binaries/libpython_codesign_verify" codesign --verify --verbose=4 "$libpython_path"
fi

if [[ -d "$site_packages" ]]; then
  native_index=1
  while IFS= read -r native_path; do
    native_id=$(printf '%03d' "$native_index")
    relative_native=${native_path#"$site_packages"/}
    capture_text "native-modules/${native_id}.path.txt" "$relative_native"
    capture_cmd "native-modules/${native_id}.file" file "$native_path"
    capture_cmd "native-modules/${native_id}.lipo" lipo -archs "$native_path"
    capture_cmd "native-modules/${native_id}.otool" otool -L "$native_path"
    capture_cmd "native-modules/${native_id}.codesign_display" codesign -dvv "$native_path"
    capture_cmd "native-modules/${native_id}.codesign_verify" codesign --verify --verbose=4 "$native_path"
    native_index=$((native_index + 1))
  done < <(find "$site_packages" -type f \( -name '*.so' -o -name '*.dylib' \) | sort)
fi

if [[ -n "$python_executable" && -d "$python_home" && -d "$site_packages" ]]; then
  run_python_probe "01-import-core-deps" "import numpy, huggingface_hub"
  run_python_probe "02-import-mlx-core" "import mlx.core"
  run_python_probe "03-import-mlx-whisper" "import mlx_whisper"
else
  capture_text "smoke-tests/skipped.txt" "Smoke tests skipped because the packaged Python runtime was incomplete."
fi

printf 'Wrote diagnostics to %s\n' "$output_dir"
