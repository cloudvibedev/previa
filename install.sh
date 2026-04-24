#!/bin/sh

set -eu

MANIFEST_URL="${MANIFEST_URL:-https://raw.githubusercontent.com/cloudvibedev/previa/main/release-metadata.json}"
PREVIA_RELEASE_BASE_URL="${PREVIA_RELEASE_BASE_URL:-https://github.com/cloudvibedev/previa/releases/download}"
PREVIA_HOME_DEFAULT="${HOME}/.previa"
PREVIA_BIN_DIR="${PREVIA_HOME_DEFAULT}/bin"
RC_BEGIN="# >>> Previa installer >>>"
RC_END="# <<< Previa installer <<<"

TEMP_DIR=""
DOWNLOADER=""
JSON_TOOL=""

setup_colors() {
  if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
    BLUE="$(printf '\033[38;5;39m')"
    GREEN="$(printf '\033[32m')"
    YELLOW="$(printf '\033[33m')"
    RED="$(printf '\033[31m')"
    BOLD="$(printf '\033[1m')"
    RESET="$(printf '\033[0m')"
  else
    BLUE=""
    GREEN=""
    YELLOW=""
    RED=""
    BOLD=""
    RESET=""
  fi
}

cleanup() {
  if [ -n "${TEMP_DIR}" ] && [ -d "${TEMP_DIR}" ]; then
    rm -rf "${TEMP_DIR}"
  fi
}

trap cleanup EXIT INT TERM

info() {
  printf "%s%s%s\n" "${BLUE}${BOLD}" "$1" "${RESET}"
}

success() {
  printf "%s%s%s\n" "${GREEN}" "$1" "${RESET}"
}

warn() {
  printf "%s%s%s\n" "${YELLOW}" "$1" "${RESET}" >&2
}

fail() {
  printf "%s%s%s\n" "${RED}" "$1" "${RESET}" >&2
  exit 1
}

print_banner() {
  cat <<'EOF'
 ++++++++++++++++++ 
+++++++++++++++++++
++++++++++++++::+++
+++++++++++++-  :+++
++++++++++=.   :+++
++++++++=.     :+++
+++++++:.  :-  :+++
+++++-.  .++-  :+++
+++=.  .=+++-  :+++
+++++++++++++++++++
 ++++++++++++++++++ 
EOF
}

require_home() {
  [ -n "${HOME:-}" ] || fail "HOME is not set."
}

detect_downloader() {
  if command -v curl >/dev/null 2>&1; then
    DOWNLOADER="curl"
  elif command -v wget >/dev/null 2>&1; then
    DOWNLOADER="wget"
  else
    fail "Missing downloader. Install curl or wget and run the installer again."
  fi
}

detect_json_tool() {
  if command -v jq >/dev/null 2>&1; then
    JSON_TOOL="jq"
  elif command -v python3 >/dev/null 2>&1; then
    JSON_TOOL="python3"
  elif command -v python >/dev/null 2>&1; then
    JSON_TOOL="python"
  else
    fail "Missing JSON parser. Install jq or python3 and run the installer again."
  fi
}

download_to() {
  url="$1"
  destination="$2"

  if [ "${DOWNLOADER}" = "curl" ]; then
    curl -fL# "${url}" -o "${destination}" || fail "Failed to download ${url}."
  else
    wget --progress=bar:force:noscroll -O "${destination}" "${url}" || fail "Failed to download ${url}."
  fi
}

manifest_value() {
  query="$1"

  if [ "${JSON_TOOL}" = "jq" ]; then
    jq -r "${query}" "${TEMP_DIR}/manifest.json"
    return
  fi

  "${JSON_TOOL}" - "$query" "${TEMP_DIR}/manifest.json" <<'PY'
import json
import pathlib
import sys

query = sys.argv[1]
path = pathlib.Path(sys.argv[2])
data = json.loads(path.read_text(encoding="utf-8"))

if query == ".version":
    value = data.get("version", "")
else:
    key = query[len('.links["'):-2]
    value = data.get("links", {}).get(key, "")

print(value)
PY
}

detect_platform() {
  os_name="${PREVIA_INSTALL_OS:-$(uname -s)}"
  arch_name="${PREVIA_INSTALL_ARCH:-$(uname -m)}"

  case "${os_name}" in
    Linux) OS_SLUG="linux" ;;
    Darwin) OS_SLUG="macos" ;;
    *) fail "Unsupported operating system: ${os_name}. Previa install script currently supports Linux and macOS." ;;
  esac

  case "${arch_name}" in
    x86_64|amd64) ARCH_SLUG="amd64" ;;
    arm64|aarch64) ARCH_SLUG="arm64" ;;
    *) fail "Unsupported architecture: ${arch_name}." ;;
  esac
}

release_asset_url() {
  asset_name="$1"
  printf "%s/v%s/%s\n" "${PREVIA_RELEASE_BASE_URL}" "${VERSION}" "${asset_name}"
}

resolve_binary_url() {
  manifest_key="$1"
  asset_name="$2"

  url="$(manifest_value ".links[\"${manifest_key}\"]")"
  if [ -n "${url}" ] && [ "${url}" != "null" ]; then
    printf "%s\n" "${url}"
    return
  fi

  case "${OS_SLUG}" in
    macos)
      warn "Manifest is missing link '${manifest_key}'. Falling back to GitHub Release asset ${asset_name}."
      release_asset_url "${asset_name}"
      ;;
    *)
      fail "Manifest is missing link '${manifest_key}'."
      ;;
  esac
}

path_contains_bin() {
  case ":${PATH}:" in
    *":${PREVIA_BIN_DIR}:"*) return 0 ;;
    *) return 1 ;;
  esac
}

update_rc_file() {
  file="$1"
  tmp_file="$(mktemp)"

  awk -v begin="${RC_BEGIN}" -v end="${RC_END}" '
    $0 == begin { skip = 1; next }
    $0 == end { skip = 0; next }
    skip != 1 { print }
  ' "${file}" > "${tmp_file}"

  {
    printf "\n%s\n" "${RC_BEGIN}"
    printf "export PREVIA_HOME=\"\$HOME/.previa\"\n"
    printf "case \":\$PATH:\" in\n"
    printf "  *\":\$PREVIA_HOME/bin:\"*) ;;\n"
    printf "  *) export PATH=\"\$PREVIA_HOME/bin:\$PATH\" ;;\n"
    printf "esac\n"
    printf "%s\n" "${RC_END}"
  } >> "${tmp_file}"

  cat "${tmp_file}" > "${file}"
  rm -f "${tmp_file}"
}

configure_shell_env() {
  export PREVIA_HOME="${PREVIA_HOME_DEFAULT}"
  if ! path_contains_bin; then
    export PATH="${PREVIA_BIN_DIR}:${PATH}"
  fi

  updated_any=0
  for rc_file in "${HOME}/.zshrc" "${HOME}/.bashrc"; do
    if [ -f "${rc_file}" ]; then
      update_rc_file "${rc_file}"
      success "Updated ${rc_file}"
      updated_any=1
    fi
  done

  if [ "${updated_any}" -eq 0 ]; then
    warn "Did not find ~/.zshrc or ~/.bashrc. Add PREVIA_HOME and PATH manually if needed."
    warn "Suggested exports:"
    warn "  export PREVIA_HOME=\"\$HOME/.previa\""
    warn "  export PATH=\"\$PREVIA_HOME/bin:\$PATH\""
  fi
}

install_binary() {
  asset_name="$1"
  local_name="$2"
  manifest_key="$3"
  target_path="${PREVIA_BIN_DIR}/${local_name}"

  url="$(resolve_binary_url "${manifest_key}" "${asset_name}")"

  info "Downloading ${local_name}"
  download_to "${url}" "${TEMP_DIR}/${asset_name}"
  chmod +x "${TEMP_DIR}/${asset_name}" || fail "Failed to mark ${local_name} as executable."
  cp "${TEMP_DIR}/${asset_name}" "${target_path}" || fail "Failed to install ${local_name} into ${PREVIA_BIN_DIR}."
  chmod +x "${target_path}" || fail "Failed to finalize permissions for ${local_name}."
  success "Installed ${local_name} -> ${target_path}"
}

main() {
  setup_colors
  require_home
  print_banner

  info "Previa installer"
  info "Detecting platform"
  detect_platform
  success "Platform: ${OS_SLUG}/${ARCH_SLUG}"

  info "Preparing installer dependencies"
  detect_downloader
  detect_json_tool
  TEMP_DIR="$(mktemp -d)"
  success "Using ${DOWNLOADER} and ${JSON_TOOL}"

  info "Downloading manifest"
  download_to "${MANIFEST_URL}" "${TEMP_DIR}/manifest.json"
  VERSION="$(manifest_value ".version")"
  [ -n "${VERSION}" ] && [ "${VERSION}" != "null" ] || fail "Manifest is invalid: missing version."
  success "Resolved latest version ${VERSION}"

  info "Installing previa into ${PREVIA_BIN_DIR}"
  mkdir -p "${PREVIA_BIN_DIR}" || fail "Failed to create ${PREVIA_BIN_DIR}."
  install_binary "previa-${OS_SLUG}-${ARCH_SLUG}" "previa" "previa_${OS_SLUG}_${ARCH_SLUG}"

  info "Configuring PREVIA_HOME and PATH"
  configure_shell_env

  success "Previa ${VERSION} installed successfully."
  printf "%sInstalled directory:%s %s\n" "${BLUE}" "${RESET}" "${PREVIA_HOME_DEFAULT}"
  printf "%sOpen a new terminal to use 'previa' from PATH.%s\n" "${BLUE}" "${RESET}"
}

main "$@"
