#!/usr/bin/env bash
set -euo pipefail

# -----------------------------------------------------------------------------
# gitar installer
# - installs into: $HOME/.gitar/bin/gitar
# - optionally updates shell rc to add $HOME/.gitar/bin to PATH
# - curl --proto '=https' --tlsv1.2 -sSf https://GITHUB_RAW/install.sh | sh
# -----------------------------------------------------------------------------

GITAR_INSTALL_DIR="${GITAR_INSTALL_DIR:-$HOME/.gitar}"
GITAR_BIN_DIR="$GITAR_INSTALL_DIR/bin"
GITAR_BIN_PATH="$GITAR_BIN_DIR/gitar"

# Set these to your GitHub org/repo
GITAR_REPO="${GITAR_REPO:-sganis/gitar}"

# If you want to pin a version, set GITAR_VERSION=v1.2.3
# Otherwise it installs the latest release.
GITAR_VERSION="${GITAR_VERSION:-latest}"

say() { printf '%s\n' "$*"; }
err() { printf 'error: %s\n' "$*" >&2; exit 1; }

need_cmd() { command -v "$1" >/dev/null 2>&1 || err "missing required command: $1"; }

detect_os() {
  local os
  os="$(uname -s | tr '[:upper:]' '[:lower:]')"
  case "$os" in
    linux) echo "linux" ;;
    darwin) echo "macos" ;;
    mingw*|msys*|cygwin*) echo "windows" ;;
    *) err "unsupported OS: $(uname -s)" ;;
  esac
}

detect_arch() {
  local arch
  arch="$(uname -m)"
  case "$arch" in
    x86_64|amd64) echo "x86_64" ;;
    aarch64|arm64) echo "aarch64" ;;
    *) err "unsupported architecture: $arch" ;;
  esac
}

download_url() {
  local os="$1" arch="$2" version="$3"

  # Artifact naming convention assumed:
  # gitar-<version>-<os>-<arch>.tar.gz  (macos/linux)
  # gitar-<version>-windows-<arch>.zip  (windows)
  #
  # Examples:
  #  gitar-v1.0.0-linux-x86_64.tar.gz
  #  gitar-v1.0.0-macos-aarch64.tar.gz
  #  gitar-v1.0.0-windows-x86_64.zip

  local base="https://github.com/$GITAR_REPO/releases"
  if [[ "$version" == "latest" ]]; then
    base="$base/latest/download"
  else
    base="$base/download/$version"
  fi

  if [[ "$os" == "windows" ]]; then
    echo "$base/gitar-${version}-windows-${arch}.zip"
  else
    echo "$base/gitar-${version}-${os}-${arch}.tar.gz"
  fi
}

install_binary_from_tar() {
  local url="$1"
  local tmpdir
  tmpdir="$(mktemp -d)"
  trap 'rm -rf "$tmpdir"' EXIT

  say "Downloading: $url"
  curl -fsSL "$url" -o "$tmpdir/gitar.tgz"

  mkdir -p "$GITAR_BIN_DIR"
  tar -xzf "$tmpdir/gitar.tgz" -C "$tmpdir"

  # Expect tar contains a single binary named "gitar"
  [[ -f "$tmpdir/gitar" ]] || err "archive did not contain a 'gitar' binary"

  install -m 0755 "$tmpdir/gitar" "$GITAR_BIN_PATH"
  say "Installed: $GITAR_BIN_PATH"
}

install_binary_from_zip() {
  local url="$1"
  local tmpdir
  tmpdir="$(mktemp -d)"
  trap 'rm -rf "$tmpdir"' EXIT

  say "Downloading: $url"
  curl -fsSL "$url" -o "$tmpdir/gitar.zip"

  mkdir -p "$GITAR_BIN_DIR"
  unzip -q "$tmpdir/gitar.zip" -d "$tmpdir"

  # Expect zip contains gitar.exe
  [[ -f "$tmpdir/gitar.exe" ]] || err "archive did not contain 'gitar.exe'"

  install -m 0755 "$tmpdir/gitar.exe" "$GITAR_BIN_DIR/gitar.exe"
  say "Installed: $GITAR_BIN_DIR/gitar.exe"
}

maybe_add_path() {
  # Add PATH line to common rc files if PATH doesn't already include it.
  local line='export PATH="$HOME/.gitar/bin:$PATH"'

  # If user already has it in PATH, skip.
  case ":$PATH:" in
    *":$HOME/.gitar/bin:"*) return 0 ;;
  esac

  # If running non-interactively, don't modify rc by default.
  if [[ "${GITAR_MODIFY_RC:-1}" != "1" ]]; then
    say "Note: not modifying shell rc (set GITAR_MODIFY_RC=1 to enable)."
    say "Add this to your shell rc:"
    say "  $line"
    return 0
  fi

  # Choose a likely rc file.
  local shell_name rc
  shell_name="$(basename "${SHELL:-sh}")"
  if [[ "$shell_name" == "zsh" ]]; then
    rc="$HOME/.zshrc"
  elif [[ "$shell_name" == "bash" ]]; then
    rc="$HOME/.bashrc"
    [[ -f "$HOME/.bash_profile" ]] && rc="$HOME/.bash_profile"
  else
    rc="$HOME/.profile"
  fi

  touch "$rc"
  if ! grep -Fq "$line" "$rc"; then
    say "Adding PATH to: $rc"
    printf '\n# gitar\n%s\n' "$line" >> "$rc"
  fi

  say "PATH updated for future shells. For this shell, run:"
  say "  export PATH=\"$HOME/.gitar/bin:\$PATH\""
}

main() {
  need_cmd curl
  local os arch url
  os="$(detect_os)"
  arch="$(detect_arch)"

  if [[ "$os" == "windows" ]]; then
    need_cmd unzip
  else
    need_cmd tar
  fi

  url="$(download_url "$os" "$arch" "$GITAR_VERSION")"
  say "Installing gitar ($GITAR_VERSION) for $os/$arch into $GITAR_INSTALL_DIR"

  if [[ "$os" == "windows" ]]; then
    install_binary_from_zip "$url"
  else
    install_binary_from_tar "$url"
  fi

  maybe_add_path

  # Smoke test
  if command -v gitar >/dev/null 2>&1; then
    say "gitar is on PATH: $(command -v gitar)"
  else
    say "gitar is not yet on PATH in this shell."
    say "Run: export PATH=\"$HOME/.gitar/bin:\$PATH\""
  fi

  say "Done. Try: gitar --version"
}

main "$@"
