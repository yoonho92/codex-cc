#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

detect_installed_codex_version() {
  local candidate
  for candidate in "${CODEX_CHANNEL_BASE_CODEX:-}" "$(command -v codex 2>/dev/null || true)" /opt/homebrew/bin/codex /usr/local/bin/codex; do
    [[ -n "${candidate}" && -x "${candidate}" ]] || continue
    "${candidate}" --version 2>/dev/null | awk '/codex-cli/ { print $2; exit }'
    return 0
  done
  return 1
}

BASE_VERSION="${CODEX_CHANNEL_BASE_VERSION:-$(detect_installed_codex_version || true)}"
if [[ -z "${BASE_VERSION}" ]]; then
  echo "Could not determine upstream Codex version." >&2
  echo "Set CODEX_CHANNEL_BASE_VERSION, for example: CODEX_CHANNEL_BASE_VERSION=0.142.5" >&2
  exit 1
fi

if [[ "${BASE_VERSION}" == "0.0.0" ]]; then
  echo "Refusing to build with version 0.0.0." >&2
  echo "Set CODEX_CHANNEL_BASE_VERSION to the upstream Codex version this fork is based on." >&2
  exit 1
fi

CC_VERSION="${CODEX_CC_VERSION:-0.1.0}"
DISPLAY_VERSION="${BASE_VERSION} (codex-cc ${CC_VERSION})"
PROFILE="${CODEX_CHANNEL_BUILD_PROFILE:-dist}"
TARGET_DIR="${CARGO_TARGET_DIR:-${CODEX_CHANNEL_CARGO_TARGET_DIR:-${REPO_ROOT}-target}}"

RUSTUP_BIN=""
if command -v rustup >/dev/null 2>&1; then
  RUSTUP_BIN="$(command -v rustup)"
elif [[ -x /opt/homebrew/bin/rustup ]]; then
  RUSTUP_BIN="/opt/homebrew/bin/rustup"
fi

if command -v cargo >/dev/null 2>&1; then
  CARGO_CMD=(cargo)
elif [[ -n "${RUSTUP_BIN}" ]]; then
  CARGO_CMD=("${RUSTUP_BIN}" run stable cargo)
else
  echo "cargo was not found. Install Rust or add cargo to PATH." >&2
  exit 1
fi

if [[ -n "${RUSTC:-}" ]]; then
  RUSTC_BIN="${RUSTC}"
elif command -v rustc >/dev/null 2>&1; then
  RUSTC_BIN="$(command -v rustc)"
elif [[ -n "${RUSTUP_BIN}" ]]; then
  RUSTC_BIN="$("${RUSTUP_BIN}" which rustc)"
else
  echo "rustc was not found. Install Rust or add rustc to PATH." >&2
  exit 1
fi

case "${PROFILE}" in
  dist)
    CARGO_ARGS=(build -p codex-cli --bin codex --profile dist)
    BINARY_PATH="${TARGET_DIR}/dist/codex"
    ;;
  release)
    CARGO_ARGS=(build -p codex-cli --bin codex --release)
    BINARY_PATH="${TARGET_DIR}/release/codex"
    ;;
  debug)
    CARGO_ARGS=(build -p codex-cli --bin codex)
    BINARY_PATH="${TARGET_DIR}/debug/codex"
    ;;
  *)
    echo "Unsupported CODEX_CHANNEL_BUILD_PROFILE=${PROFILE}. Use dist, release, or debug." >&2
    exit 1
    ;;
esac

echo "Building Codex-CC ${CC_VERSION} on Codex ${BASE_VERSION}"
echo "Target dir: ${TARGET_DIR}"

(
  cd "${REPO_ROOT}/codex-rs"
  CARGO_TARGET_DIR="${TARGET_DIR}" \
    CODEX_CLI_VERSION_OVERRIDE="${BASE_VERSION}" \
    CODEX_CC_VERSION="${CC_VERSION}" \
    CODEX_CC_DISPLAY_VERSION="${DISPLAY_VERSION}" \
    CODEX_DISTRIBUTION="codex-cc" \
    RUSTC="${RUSTC_BIN}" \
    "${CARGO_CMD[@]}" "${CARGO_ARGS[@]}"
)

if [[ ! -x "${BINARY_PATH}" ]]; then
  echo "Build finished but binary was not found at ${BINARY_PATH}" >&2
  exit 1
fi

echo "Built: ${BINARY_PATH}"
"${BINARY_PATH}" --version
