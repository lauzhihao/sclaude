#!/usr/bin/env bash
set -euo pipefail

REPO="${SCLAUDE_REPO:-lauzhihao/sclaude}"
SCLAUDE_HOME="${SCLAUDE_HOME:-${HOME}/.sclaude}"
export SCLAUDE_HOME
INSTALL_BIN="${INSTALL_BIN:-${SCLAUDE_HOME}/bin}"
TMP_ROOT="${SCLAUDE_HOME}/tmp"
WRAPPER_PATH="${INSTALL_BIN}/sclaude"
OPUS_PATH="${INSTALL_BIN}/opus"
SONNET_PATH="${INSTALL_BIN}/sonnet"
HAIKU_PATH="${INSTALL_BIN}/haiku"
ORIGINAL_WRAPPER_PATH="${INSTALL_BIN}/sclaude-original"
VERSION="${SCLAUDE_VERSION:-}"

need_cmd() {
  command -v "$1" >/dev/null 2>&1
}

show_requirements() {
  local missing=0
  local cmd
  echo "Dependency check:"
  for cmd in bash curl tar mktemp; do
    if need_cmd "${cmd}"; then
      printf '  [ok] %s -> %s\n' "${cmd}" "$(command -v "${cmd}")"
    else
      printf '  [missing] %s\n' "${cmd}" >&2
      missing=1
    fi
  done
  if [[ "${missing}" -ne 0 ]]; then
    echo "Install aborted because required commands are missing." >&2
    exit 1
  fi
}

detect_target() {
  local os arch
  os="$(uname -s 2>/dev/null || echo unknown)"
  arch="$(uname -m 2>/dev/null || echo unknown)"

  case "${os}/${arch}" in
    Darwin/arm64|Darwin/aarch64)
      echo "aarch64-apple-darwin"
      ;;
    Darwin/x86_64)
      echo "x86_64-apple-darwin"
      ;;
    Linux/x86_64|Linux/amd64)
      echo "x86_64-unknown-linux-musl"
      ;;
    *)
      echo "Unsupported platform: ${os}/${arch}" >&2
      echo "Use a published release asset manually or build from source with cargo." >&2
      exit 1
      ;;
  esac
}

resolve_version() {
  if [[ -n "${VERSION}" ]]; then
    echo "${VERSION}"
    return 0
  fi

  local api_url
  api_url="https://api.github.com/repos/${REPO}/releases/latest"
  VERSION="$(
    curl -fsSL "${api_url}" \
      | sed -n 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' \
      | head -n 1
  )"
  if [[ -z "${VERSION}" ]]; then
    echo "Failed to resolve latest release tag from ${api_url}" >&2
    exit 1
  fi
  echo "${VERSION}"
}

download_and_install() {
  local version target asset url tmp_dir cleanup_dir archive_path extracted_path
  version="$1"
  target="$2"
  asset="sclaude-${version}-${target}.tar.gz"
  url="https://github.com/${REPO}/releases/download/${version}/${asset}"
  mkdir -p "${TMP_ROOT}"
  tmp_dir="$(mktemp -d "${TMP_ROOT}/install.XXXXXX")"
  cleanup_dir="${tmp_dir}"
  trap 'rm -rf -- "'"${cleanup_dir}"'"' EXIT
  archive_path="${tmp_dir}/${asset}"

  echo "Downloading ${url}"
  curl -fsSL "${url}" -o "${archive_path}"

  mkdir -p "${INSTALL_BIN}"
  tar -xzf "${archive_path}" -C "${tmp_dir}"
  extracted_path="${tmp_dir}/sclaude"
  if [[ ! -f "${extracted_path}" ]]; then
    echo "Release archive did not contain a top-level sclaude binary." >&2
    exit 1
  fi

  install -m 0755 "${extracted_path}" "${WRAPPER_PATH}"
  cp "${WRAPPER_PATH}" "${OPUS_PATH}"
  cp "${WRAPPER_PATH}" "${SONNET_PATH}"
  cp "${WRAPPER_PATH}" "${HAIKU_PATH}"
}

install_original_wrapper() {
  cat > "${ORIGINAL_WRAPPER_PATH}" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
SCLAUDE_HOME="${SCLAUDE_HOME:-${HOME}/.sclaude}"
if [[ -n "${CLAUDE_BIN:-}" && -x "${CLAUDE_BIN}" ]]; then
  exec "${CLAUDE_BIN}" "$@"
fi
runtime_claude="${SCLAUDE_HOME}/runtime/claude-code/bin/claude"
if [[ -x "${runtime_claude}" ]]; then
  exec "${runtime_claude}" "$@"
fi
runtime_claude="${SCLAUDE_HOME}/runtime/claude-code/node_modules/.bin/claude"
if [[ -x "${runtime_claude}" ]]; then
  exec "${runtime_claude}" "$@"
fi
if command -v claude >/dev/null 2>&1; then
  exec "$(command -v claude)" "$@"
fi
echo "claude not found on PATH." >&2
exit 1
EOF
  chmod 0755 "${ORIGINAL_WRAPPER_PATH}"
}

post_install_import() {
  if [[ -f "${HOME}/.claude.json" || -f "${HOME}/.config.json" || -f "${HOME}/.claude/.claude.json" || -f "${HOME}/.claude/.config.json" ]]; then
    if "${WRAPPER_PATH}" import-known >/dev/null 2>&1; then
      echo "Imported current Claude profile into sclaude state."
      if "${WRAPPER_PATH}" refresh >/dev/null 2>&1; then
        echo "Refreshed sclaude login status cache."
      else
        echo "Imported profile, but refreshing status cache failed." >&2
      fi
    else
      echo "Installed sclaude, but importing the current Claude profile failed." >&2
    fi
  else
    echo "No Claude login state found under \$HOME/.claude or \$HOME/*.json; skipped import."
  fi
}

print_next_steps() {
  echo "SCLAUDE_HOME is ${SCLAUDE_HOME}"
  echo "Installed to ${WRAPPER_PATH}"
  echo "Installed model entrypoints to ${OPUS_PATH}, ${SONNET_PATH}, ${HAIKU_PATH}"
  echo "Installed passthrough helper to ${ORIGINAL_WRAPPER_PATH}"
  if [[ ":$PATH:" != *":${INSTALL_BIN}:"* ]]; then
    echo
    echo "${INSTALL_BIN} is not currently on PATH."
    echo "Add this line to your shell profile:"
    echo "  export PATH=\"${INSTALL_BIN}:\$PATH\""
  fi
}

show_requirements
TARGET="$(detect_target)"
VERSION="$(resolve_version)"
download_and_install "${VERSION}" "${TARGET}"
install_original_wrapper
post_install_import
print_next_steps
