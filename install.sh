#!/usr/bin/env bash
set -euo pipefail

# -----------------------------------------------------------------------------
# gitar installer (Linux/macOS)
# Usage:
#   curl -fsSL https://<YOUR_URL>/install.sh | bash
#   curl -fsSL https://<YOUR_URL>/install.sh | bash -s -- latest
#   curl -fsSL https://<YOUR_URL>/install.sh | bash -s -- v1.2.3
#
# Behavior (Claude-code style):
# - Detects platform (linux/macos + x64/arm64)
# - Resolves version (default: latest)
# - Downloads release artifact from GitHub Releases
# - Verifies sha256 if a .sha256 sidecar exists (optional but recommended)
# - Installs to: $HOME/.gitar/bin/gitar (or $GITAR_INSTALL_DIR)
# - Adds PATH export to shell rc (can be disabled)
# -----------------------------------------------------------------------------

TARGET="${1:-}" # optional: latest|stable|vX.Y.Z
if [[ -n "$TARGET" ]] && [[ ! "$TARGET" =~ ^(stable|latest|v[0-9]+\.[0-9]+\.[0-9]+(-[^[:space:]]+)?)$ ]]; then
  echo "Usage: $0 [stable|latest|vX.Y.Z]" >&2
  exit 1
fi

GITAR_REPO="${GITAR_REPO:-sganis/gitar}"
GITAR_INSTALL_DIR="${GITAR_INSTALL_DIR:-$HOME/.gitar}"
GITAR_BIN_DIR="$GITAR_INSTALL_DIR/bin"
GITAR_BIN_PATH="$GITAR_BIN_DIR/gitar"

# Set to 0 to avoid modifying rc files
GITAR_MODIFY_RC="${GITAR_MODIFY_RC:-1}"

# ----------------------------
# Helpers
# ----------------------------
say() { printf '%s\n' "$*"; }
err() { printf 'error: %s\n' "$*" >&2; exit 1; }

DOWNLOADER=""
if command -v curl >/dev/null 2>&1; then
  DOWNLOADER="curl"
elif command -v wget >/dev/null 2>&1; then
  DOWNLOADER="wget"
else
  err "Either curl or wget is required but neither is installed"
fi

need_cmd() { command -v "$1" >/dev/null 2>&1 || err "missing required command: $1"; }

download_file() {
  local url="$1"
  local output="${2:-}"
  if [[ "$DOWNLOADER" == "curl" ]]; then
    if [[ -n "$output" ]]; then
      curl -fsSL -o "$output" "$url"
    else
      curl -fsSL "$url"
    fi
  else
    if [[ -n "$output" ]]; then
      wget -q -O "$output" "$url"
    else
      wget -q -O - "$url"
    fi
  fi
}

detect_os() {
  case "$(uname -s)" in
    Darwin) echo "macos" ;;
    Linux)  echo "linux" ;;
    *) err "Unsupported OS: $(uname -s) (Windows not supported by this installer)" ;;
  esac
}

detect_arch() {
  case "$(uname -m)" in
    x86_64|amd64) echo "x86_64" ;;
    arm64|aarch64) echo "aarch64" ;;
    *) err "Unsupported architecture: $(uname -m)" ;;
  esac
}

# Best-effort JSON parse without jq:
# Extracts the first "tag_name":"vX.Y.Z" from GitHub API response.
extract_tag_name() {
  local json="$1"
  # Normalize to one line to simplify.
  json="$(echo "$json" | tr -d '\n\r\t')"
  if [[ "$json" =~ \"tag_name\"[[:space:]]*:[[:space:]]*\"([^\"]+)\" ]]; then
    echo "${BASH_REMATCH[1]}"
    return 0
  fi
  return 1
}

resolve_version() {
  local target="${1:-}"
  if [[ -z "$target" || "$target" == "latest" || "$target" == "stable" ]]; then
    # Use GitHub Releases API to get latest tag
    local api="https://api.github.com/repos/$GITAR_REPO/releases/latest"
    local json
    json="$(download_file "$api")" || err "Failed to query GitHub API for latest release"
    local tag
    tag="$(extract_tag_name "$json")" || err "Could not parse latest release tag from GitHub API"
    echo "$tag"
  else
    # already validated like v1.2.3
    echo "$target"
  fi
}

artifact_url() {
  local version="$1" os="$2" arch="$3"
  # Your current convention:
  # gitar-<version>-<os>-<arch>.tar.gz
  # Example: gitar-v1.0.0-linux-x86_64.tar.gz
  echo "https://github.com/$GITAR_REPO/releases/download/$version/gitar-${version}-${os}-${arch}.tar.gz"
}

artifact_sha_url() {
  local version="$1" os="$2" arch="$3"
  # Optional but recommended to upload alongside the tarball:
  # gitar-<version>-<os>-<arch>.tar.gz.sha256
  echo "https://github.com/$GITAR_REPO/releases/download/$version/gitar-${version}-${os}-${arch}.tar.gz.sha256"
}

sha256_file_hash() {
  local file="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file" | awk '{print $1}'
  else
    err "Missing sha256 tool: install sha256sum or shasum"
  fi
}

maybe_add_path() {
  local line='export PATH="$HOME/.gitar/bin:$PATH"'

  case ":$PATH:" in
    *":$HOME/.gitar/bin:"*) return 0 ;;
  esac

  if [[ "$GITAR_MODIFY_RC" != "1" ]]; then
    say "Note: not modifying shell rc (set GITAR_MODIFY_RC=1 to enable)."
    say "Add this to your shell rc:"
    say "  $line"
    return 0
  fi

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
}

# ----------------------------
# Main
# ----------------------------
main() {
  need_cmd tar
  need_cmd mktemp

  local os arch version url sha_url
  os="$(detect_os)"
  arch="$(detect_arch)"
  version="$(resolve_version "$TARGET")"

  url="$(artifact_url "$version" "$os" "$arch")"
  sha_url="$(artifact_sha_url "$version" "$os" "$arch")"

  say "Installing gitar ($version) for $os/$arch"
  say "Repo: $GITAR_REPO"
  say "Install dir: $GITAR_INSTALL_DIR"
  say ""

  local tmpdir tgz shafile expected actual
  tmpdir="$(mktemp -d)"
  trap 'rm -rf "$tmpdir"' EXIT

  tgz="$tmpdir/gitar.tgz"
  shafile="$tmpdir/gitar.tgz.sha256"

  say "Downloading: $url"
  if ! download_file "$url" "$tgz"; then
    err "Download failed (asset not found for $os/$arch?). URL: $url"
  fi

  # Optional checksum verification (if .sha256 exists)
  expected=""
  if download_file "$sha_url" "$shafile" 2>/dev/null; then
    # accept formats:
    #  <hash>
    #  <hash>  filename
    expected="$(awk '{print $1}' < "$shafile" | tr -d '\r\n' | head -n1)"
    if [[ "$expected" =~ ^[a-fA-F0-9]{64}$ ]]; then
      actual="$(sha256_file_hash "$tgz")"
      if [[ "${actual,,}" != "${expected,,}" ]]; then
        err "Checksum verification failed"
      fi
      say "Checksum OK"
    else
      say "Warning: checksum file found but format not recognized; skipping verification"
    fi
  else
    say "Note: no checksum file found; skipping verification."
    say "      (Recommended: upload *.tar.gz.sha256 with SHA-256 of the tarball.)"
  fi

  mkdir -p "$GITAR_BIN_DIR"
  tar -xzf "$tgz" -C "$tmpdir"

  [[ -f "$tmpdir/gitar" ]] || err "Archive did not contain a 'gitar' binary at root"
  install -m 0755 "$tmpdir/gitar" "$GITAR_BIN_PATH"
  say "Installed: $GITAR_BIN_PATH"

  maybe_add_path

  # Smoke test (without requiring current shell PATH modification)
  say ""
  if "$GITAR_BIN_PATH" --version >/dev/null 2>&1; then
    say "✅ gitar runs: $("$GITAR_BIN_PATH" --version 2>/dev/null || true)"
  else
    say "✅ gitar installed"
  fi

  if command -v gitar >/dev/null 2>&1; then
    say "✅ gitar is on PATH: $(command -v gitar)"
  else
    say "ℹ️  gitar is not yet on PATH in this shell."
    say "   Run: export PATH=\"$HOME/.gitar/bin:\$PATH\""
  fi

  say ""
  say "✅ Installation complete!"
  say ""
}

main "$@"
