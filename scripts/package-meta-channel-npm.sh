#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

SCOPE="${CODEX_CHANNEL_NPM_SCOPE:-@yoonho92}"
PACKAGE_BASENAME="${CODEX_CHANNEL_NPM_NAME:-codex-cc}"
BIN_NAME="${CODEX_CHANNEL_BIN_NAME:-codex-cc}"
OUTPUT_DIR="${CODEX_CHANNEL_NPM_OUT:-${REPO_ROOT}/dist/npm/meta-channel}"

case "$(uname -s)" in
  Darwin)
    OS_NAME="darwin"
    case "$(uname -m)" in
      arm64) CPU_NAME="arm64"; TARGET_TRIPLE="aarch64-apple-darwin" ;;
      x86_64) CPU_NAME="x64"; TARGET_TRIPLE="x86_64-apple-darwin" ;;
      *) echo "Unsupported macOS arch: $(uname -m)" >&2; exit 1 ;;
    esac
    ;;
  Linux)
    OS_NAME="linux"
    case "$(uname -m)" in
      aarch64|arm64) CPU_NAME="arm64"; TARGET_TRIPLE="aarch64-unknown-linux-musl" ;;
      x86_64) CPU_NAME="x64"; TARGET_TRIPLE="x86_64-unknown-linux-musl" ;;
      *) echo "Unsupported Linux arch: $(uname -m)" >&2; exit 1 ;;
    esac
    ;;
  *)
    echo "Unsupported OS: $(uname -s). Build Windows packages on Windows." >&2
    exit 1
    ;;
esac

TARGET_TAG="${OS_NAME}-${CPU_NAME}"
WRAPPER_PACKAGE="${SCOPE}/${PACKAGE_BASENAME}"
PLATFORM_PACKAGE="${SCOPE}/${PACKAGE_BASENAME}-${TARGET_TAG}"

if [[ -n "${CODEX_CHANNEL_BINARY:-}" ]]; then
  CODEX_BINARY="${CODEX_CHANNEL_BINARY}"
elif [[ -x "${REPO_ROOT}/codex-rs/target/dist/codex" ]]; then
  CODEX_BINARY="${REPO_ROOT}/codex-rs/target/dist/codex"
elif [[ -x "${REPO_ROOT}-target/dist/codex" ]]; then
  CODEX_BINARY="${REPO_ROOT}-target/dist/codex"
elif [[ -x "${REPO_ROOT}/codex-rs/target/release/codex" ]]; then
  CODEX_BINARY="${REPO_ROOT}/codex-rs/target/release/codex"
elif [[ -x "${REPO_ROOT}-target/release/codex" ]]; then
  CODEX_BINARY="${REPO_ROOT}-target/release/codex"
elif [[ "${CODEX_CHANNEL_ALLOW_DEBUG_BINARY:-0}" == "1" && -x "${REPO_ROOT}/codex-rs/target/debug/codex" ]]; then
  CODEX_BINARY="${REPO_ROOT}/codex-rs/target/debug/codex"
elif [[ -x "${REPO_ROOT}-target/debug/codex" ]]; then
  if [[ "${CODEX_CHANNEL_ALLOW_DEBUG_BINARY:-0}" != "1" ]]; then
    echo "Only a debug Codex binary was found: ${REPO_ROOT}-target/debug/codex" >&2
    echo "Build a dist/release binary first:" >&2
    echo "  ${REPO_ROOT}/scripts/build-meta-channel-codex.sh" >&2
    echo "Or set CODEX_CHANNEL_ALLOW_DEBUG_BINARY=1 for local testing." >&2
    exit 1
  fi
  CODEX_BINARY="${REPO_ROOT}-target/debug/codex"
else
  echo "Codex binary not found." >&2
  echo "Build first: ${REPO_ROOT}/scripts/build-meta-channel-codex.sh" >&2
  echo "Or set CODEX_CHANNEL_BINARY=/absolute/path/to/codex." >&2
  exit 1
fi

if ! command -v npm >/dev/null 2>&1; then
  echo "npm is required to pack the distribution tarballs." >&2
  exit 1
fi

parse_codex_version() {
  "$1" --version 2>/dev/null | awk '/codex-cli/ { print $2; exit }'
}

BINARY_VERSION="$(parse_codex_version "${CODEX_BINARY}")"
if [[ -z "${BINARY_VERSION}" ]]; then
  echo "Could not determine Codex binary version from: ${CODEX_BINARY}" >&2
  exit 1
fi

if [[ "${BINARY_VERSION}" == "0.0.0" && "${CODEX_CHANNEL_ALLOW_ZERO_BINARY_VERSION:-0}" != "1" ]]; then
  echo "Selected Codex binary reports version 0.0.0: ${CODEX_BINARY}" >&2
  echo "Build a versioned binary first:" >&2
  echo "  ${REPO_ROOT}/scripts/build-meta-channel-codex.sh" >&2
  echo "Or set CODEX_CHANNEL_ALLOW_ZERO_BINARY_VERSION=1 to package it anyway." >&2
  exit 1
fi

SHORT_SHA="$(git -C "${REPO_ROOT}" rev-parse --short HEAD 2>/dev/null || echo local)"
BASE_VERSION="${CODEX_CHANNEL_BASE_VERSION:-${BINARY_VERSION}}"
if [[ "${BASE_VERSION}" == *-* ]]; then
  DEFAULT_VERSION="${BASE_VERSION}.cc.${SHORT_SHA}"
else
  DEFAULT_VERSION="${BASE_VERSION}-cc.${SHORT_SHA}"
fi
VERSION="${CODEX_CHANNEL_VERSION:-${DEFAULT_VERSION}}"
PLATFORM_VERSION="${VERSION}"

STAGE_ROOT="${OUTPUT_DIR}/stage"
WRAPPER_STAGE="${STAGE_ROOT}/wrapper"
PLATFORM_STAGE="${STAGE_ROOT}/platform"
rm -rf "${STAGE_ROOT}"
mkdir -p \
  "${WRAPPER_STAGE}/bin" \
  "${WRAPPER_STAGE}/vendor/${TARGET_TRIPLE}/codex" \
  "${PLATFORM_STAGE}/vendor/${TARGET_TRIPLE}/codex" \
  "${OUTPUT_DIR}"

install -m 0755 "${CODEX_BINARY}" "${WRAPPER_STAGE}/vendor/${TARGET_TRIPLE}/codex/codex"
install -m 0755 "${CODEX_BINARY}" "${PLATFORM_STAGE}/vendor/${TARGET_TRIPLE}/codex/codex"

if [[ -n "${CODEX_CHANNEL_RG_BINARY:-}" && -x "${CODEX_CHANNEL_RG_BINARY}" ]]; then
  mkdir -p "${WRAPPER_STAGE}/vendor/${TARGET_TRIPLE}/path" "${PLATFORM_STAGE}/vendor/${TARGET_TRIPLE}/path"
  install -m 0755 "${CODEX_CHANNEL_RG_BINARY}" "${WRAPPER_STAGE}/vendor/${TARGET_TRIPLE}/path/rg"
  install -m 0755 "${CODEX_CHANNEL_RG_BINARY}" "${PLATFORM_STAGE}/vendor/${TARGET_TRIPLE}/path/rg"
elif command -v rg >/dev/null 2>&1; then
  mkdir -p "${WRAPPER_STAGE}/vendor/${TARGET_TRIPLE}/path" "${PLATFORM_STAGE}/vendor/${TARGET_TRIPLE}/path"
  install -m 0755 "$(command -v rg)" "${WRAPPER_STAGE}/vendor/${TARGET_TRIPLE}/path/rg"
  install -m 0755 "$(command -v rg)" "${PLATFORM_STAGE}/vendor/${TARGET_TRIPLE}/path/rg"
fi

cat > "${WRAPPER_STAGE}/bin/${BIN_NAME}.js" <<EOF_JS
#!/usr/bin/env node
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const require = createRequire(import.meta.url);

const targetTriple = "${TARGET_TRIPLE}";
const platformPackage = "${PLATFORM_PACKAGE}";
const binaryName = process.platform === "win32" ? "codex.exe" : "codex";
const localVendorRoot = path.join(__dirname, "..", "vendor");
const localBinaryPath = path.join(localVendorRoot, targetTriple, "codex", binaryName);

let vendorRoot = null;
if (existsSync(localBinaryPath)) {
  vendorRoot = localVendorRoot;
} else {
  try {
    const packageJsonPath = require.resolve(\`\${platformPackage}/package.json\`);
    vendorRoot = path.join(path.dirname(packageJsonPath), "vendor");
  } catch {
    throw new Error(
      \`Missing native payload for \${platformPackage}. Reinstall ${WRAPPER_PACKAGE} or install the matching platform package.\`,
    );
  }
}

const archRoot = path.join(vendorRoot, targetTriple);
const binaryPath = path.join(archRoot, "codex", binaryName);
const pathDir = path.join(archRoot, "path");
const pathSep = process.platform === "win32" ? ";" : ":";
const env = {
  ...process.env,
  PATH: existsSync(pathDir) ? \`\${pathDir}\${pathSep}\${process.env.PATH || ""}\` : process.env.PATH,
  CODEX_META_CHANNEL_NPM: "1",
};

const child = spawn(binaryPath, process.argv.slice(2), { stdio: "inherit", env });
child.on("error", (err) => {
  console.error(err);
  process.exit(1);
});
for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"]) {
  process.on(signal, () => {
    if (!child.killed) child.kill(signal);
  });
}
child.on("exit", (code, signal) => {
  if (signal) process.kill(process.pid, signal);
  else process.exit(code ?? 1);
});
EOF_JS
chmod 0755 "${WRAPPER_STAGE}/bin/${BIN_NAME}.js"

cat > "${WRAPPER_STAGE}/README.md" <<EOF_MD
# ${WRAPPER_PACKAGE}

Codex-CC is a Codex CLI build with a typed channel ingress core patch.
It lets trusted local integrations surface inbound messages inside Codex without
terminal scrollback writes or keyboard input emulation.

- Command: \`${BIN_NAME}\`
- Embedded Codex app version: \`${BINARY_VERSION}\`
- Package version: \`${VERSION}\`

Install:

\`\`\`bash
npm install -g ${WRAPPER_PACKAGE}
${BIN_NAME} --version
${BIN_NAME}
\`\`\`

This package intentionally exposes \`${BIN_NAME}\` instead of overwriting the
official \`codex\` command. Add your own shell alias if you want it to be the
default Codex executable.

## What is included

- \`thread/channel_append\`: append trusted inbound channel messages.
- \`thread/channel/appended\`: live app-server notification for TUI rendering.
- \`ChannelMessage\`: durable thread item for replay, resume, and thread reads.
- Delivery modes: \`surfaceOnly\` for display-only messages, or
  \`surfaceAndQueueNextTurn\` for integrations that intentionally queue a
  distilled payload for the next model turn.
- MCP/app notification parsing for channel payloads using fields like
  \`text\`, \`channel\`, \`sender\`, \`priority\`, \`delivery\`, and optional
  \`modelText\`.

When \`surfaceAndQueueNextTurn\` is used without \`modelText\`, Codex queues
metadata only. Display \`text\` and \`preview\` are not copied into model
context by default.

Codex-CC is transport-neutral. External chat relays, peer-agent bridges, local
app bridges, and other tools can use the same ingress lane without being baked
into the core.

Build the binary with \`scripts/build-meta-channel-codex.sh\` before packaging.
That script injects the upstream Codex version at compile time, so the TUI and
\`${BIN_NAME} --version\` do not show the local development version \`0.0.0\`.
The default build profile is \`dist\`; use \`CODEX_CHANNEL_BUILD_PROFILE=release\`
only when you need the smallest upstream-style binary.
EOF_MD

cat > "${WRAPPER_STAGE}/package.json" <<EOF_JSON
{
  "name": "${WRAPPER_PACKAGE}",
  "version": "${VERSION}",
  "license": "Apache-2.0",
  "type": "module",
  "description": "Codex CLI with the meta-channel ingress core patch.",
  "bin": {
    "${BIN_NAME}": "bin/${BIN_NAME}.js"
  },
  "files": [
    "bin",
    "vendor",
    "README.md"
  ],
  "optionalDependencies": {
    "${PLATFORM_PACKAGE}": "npm:${PLATFORM_PACKAGE}@${PLATFORM_VERSION}"
  },
  "engines": {
    "node": ">=18"
  }
}
EOF_JSON

cat > "${PLATFORM_STAGE}/README.md" <<EOF_MD
# ${PLATFORM_PACKAGE}

Native payload for ${WRAPPER_PACKAGE} on ${TARGET_TAG}.
EOF_MD

cat > "${PLATFORM_STAGE}/package.json" <<EOF_JSON
{
  "name": "${PLATFORM_PACKAGE}",
  "version": "${PLATFORM_VERSION}",
  "license": "Apache-2.0",
  "description": "Native Codex meta-channel payload for ${TARGET_TAG}.",
  "os": ["${OS_NAME}"],
  "cpu": ["${CPU_NAME}"],
  "files": [
    "vendor",
    "README.md"
  ],
  "engines": {
    "node": ">=18"
  }
}
EOF_JSON

npm pack --pack-destination "${OUTPUT_DIR}" "${PLATFORM_STAGE}" >/dev/null
WRAPPER_TGZ="$(npm pack --pack-destination "${OUTPUT_DIR}" "${WRAPPER_STAGE}")"

echo "Packaged ${WRAPPER_PACKAGE}@${VERSION}"
echo "Binary: ${CODEX_BINARY}"
echo "Binary version: ${BINARY_VERSION}"
echo "Output:"
find "${OUTPUT_DIR}" -maxdepth 1 -type f -name '*.tgz' -print | sort
echo
echo "Local install test:"
echo "  npm install -g ${OUTPUT_DIR}/${WRAPPER_TGZ}"
echo "  ${BIN_NAME} --version"
