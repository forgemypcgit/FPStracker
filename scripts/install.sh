#!/usr/bin/env bash
set -euo pipefail

REPO="${FPS_TRACKER_REPO:-forgemypcgit/FPStracker}"
BINARY_NAME="fps-tracker"
INSTALL_DIR_DEFAULT="${HOME}/.local/bin"
INSTALL_DIR="${FPS_TRACKER_INSTALL_DIR:-$INSTALL_DIR_DEFAULT}"
COSIGN_VERSION="${FPS_TRACKER_COSIGN_VERSION:-v2.4.1}"
COSIGN_PUBKEY_OVERRIDE="${FPS_TRACKER_COSIGN_PUBKEY:-}"
SKIP_SIG_VERIFY="${FPS_TRACKER_SKIP_SIGNATURE_VERIFY:-0}"
REQUIRE_SIG_VERIFY="${FPS_TRACKER_REQUIRE_SIGNATURE_VERIFY:-0}"
SKIP_COSIGN_CHECKSUM_VERIFY="${FPS_TRACKER_SKIP_COSIGN_CHECKSUM_VERIFY:-0}"
BASE_URL_OVERRIDE="${FPS_TRACKER_BASE_URL:-}"

if ! command -v curl >/dev/null 2>&1; then
  echo "curl is required for installation" >&2
  exit 1
fi

curl_download() {
  local url out
  url="$1"
  out="$2"
  if [[ "$url" == http://* ]]; then
    if [[ "$url" =~ ^http://(127\.0\.0\.1|localhost)(:[0-9]+)?/ ]] \
      || [[ "${FPS_TRACKER_ALLOW_INSECURE_HTTP:-0}" == "1" ]]; then
      curl -fL --retry 3 --retry-delay 1 -sS "$url" -o "$out"
      return 0
    fi

    echo "Refusing insecure HTTP download: $url" >&2
    echo "Use https, or set FPS_TRACKER_ALLOW_INSECURE_HTTP=1 (not recommended)." >&2
    return 1
  fi

  curl --proto '=https' --tlsv1.2 -fL --retry 3 --retry-delay 1 -sS "$url" -o "$out"
}

curl_text() {
  local url
  url="$1"
  if [[ "$url" == http://* ]]; then
    if [[ "$url" =~ ^http://(127\.0\.0\.1|localhost)(:[0-9]+)?/ ]] \
      || [[ "${FPS_TRACKER_ALLOW_INSECURE_HTTP:-0}" == "1" ]]; then
      curl -fsSL --retry 3 --retry-delay 1 -sS "$url"
      return 0
    fi

    echo "Refusing insecure HTTP request: $url" >&2
    echo "Use https, or set FPS_TRACKER_ALLOW_INSECURE_HTTP=1 (not recommended)." >&2
    return 1
  fi

  curl --proto '=https' --tlsv1.2 -fsSL --retry 3 --retry-delay 1 -sS "$url"
}

detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux)
      case "$arch" in
        x86_64|amd64) echo "x86_64-unknown-linux-gnu" ;;
        *)
          echo "Unsupported Linux architecture: $arch" >&2
          return 1
          ;;
      esac
      ;;
    Darwin)
      case "$arch" in
        x86_64|amd64) echo "x86_64-apple-darwin" ;;
        arm64|aarch64) echo "aarch64-apple-darwin" ;;
        *)
          echo "Unsupported macOS architecture: $arch" >&2
          return 1
          ;;
      esac
      ;;
    *)
      echo "Unsupported OS: $os" >&2
      return 1
      ;;
  esac
}

cosign_asset_name() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux)
      case "$arch" in
        x86_64|amd64) echo "cosign-linux-amd64" ;;
        *) return 1 ;;
      esac
      ;;
    Darwin)
      case "$arch" in
        x86_64|amd64) echo "cosign-darwin-amd64" ;;
        arm64|aarch64) echo "cosign-darwin-arm64" ;;
        *) return 1 ;;
      esac
      ;;
    *) return 1 ;;
  esac
}

resolve_version() {
  if [[ -n "$BASE_URL_OVERRIDE" ]]; then
    if [[ -n "${1:-}" ]]; then
      echo "$1"
    else
      echo "custom"
    fi
    return 0
  fi

  if [[ "${1:-}" != "" ]]; then
    echo "$1"
    return 0
  fi

  local tag
  tag="$(curl_text "https://api.github.com/repos/${REPO}/releases/latest" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1)"

  if [[ -z "$tag" ]]; then
    echo "Could not resolve latest release tag. Set FPS_TRACKER_VERSION manually." >&2
    return 1
  fi

  echo "$tag"
}

sha256_file() {
  local file
  file="$1"

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{print $1}'
    return 0
  fi

  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file" | awk '{print $1}'
    return 0
  fi

  if command -v python3 >/dev/null 2>&1; then
    python3 - "$file" <<'PY'
import hashlib
import sys
from pathlib import Path
path = Path(sys.argv[1])
h = hashlib.sha256()
with path.open("rb") as f:
    for chunk in iter(lambda: f.read(1024 * 1024), b""):
        h.update(chunk)
print(h.hexdigest())
PY
    return 0
  fi

  echo "No SHA256 tool available (sha256sum/shasum/python3)." >&2
  return 1
}

verify_checksum() {
  local file checksum_file
  file="$1"
  checksum_file="$2"

  local expected actual
  expected="$(awk '{print $1}' "$checksum_file" | head -n1 | tr '[:upper:]' '[:lower:]')"
  if [[ -z "$expected" ]]; then
    echo "Checksum file is empty: $checksum_file" >&2
    return 1
  fi

  actual="$(sha256_file "$file" | tr '[:upper:]' '[:lower:]')"
  if [[ -z "$actual" ]]; then
    echo "Failed to compute SHA256 for: $file" >&2
    return 1
  fi

  if [[ "$expected" != "$actual" ]]; then
    echo "Checksum mismatch for $(basename "$file")" >&2
    echo "  expected: $expected" >&2
    echo "  actual:   $actual" >&2
    return 1
  fi
}

expected_cosign_sha256() {
  local version asset
  version="$1"
  asset="$2"

  # Pinned checksums for cosign v2.4.1 (to avoid trusting an unverified downloader).
  if [[ "$version" != "v2.4.1" ]]; then
    return 0
  fi

  case "$asset" in
    cosign-linux-amd64) echo "8b24b946dd5809c6bd93de08033bcf6bc0ed7d336b7785787c080f574b89249b" ;;
    cosign-darwin-amd64) echo "666032ca283da92b6f7953965688fd51200fdc891a86c19e05c98b898ea0af4e" ;;
    cosign-darwin-arm64) echo "13343856b69f70388c4fe0b986a31dde5958e444b41be22d785d3dc5e1a9cc62" ;;
    *) return 1 ;;
  esac
}

ensure_cosign() {
  local tmp_dir="$1"

  if command -v cosign >/dev/null 2>&1; then
    echo "cosign"
    return 0
  fi

  local asset
  asset="$(cosign_asset_name)" || {
    echo "Could not determine cosign asset for this platform." >&2
    return 1
  }

  local url
  url="https://github.com/sigstore/cosign/releases/download/${COSIGN_VERSION}/${asset}"

  local destination="${tmp_dir}/cosign"
  curl_download "$url" "$destination"
  chmod +x "$destination"

  local expected actual
  expected="$(expected_cosign_sha256 "$COSIGN_VERSION" "$asset" || true)"
  if [[ -z "$expected" ]]; then
    if [[ "$SKIP_COSIGN_CHECKSUM_VERIFY" == "1" ]]; then
      echo "Warning: cosign checksum verification skipped (FPS_TRACKER_SKIP_COSIGN_CHECKSUM_VERIFY=1)." >&2
    else
      echo "No pinned checksum available for cosign ${COSIGN_VERSION} (${asset})." >&2
      echo "Set FPS_TRACKER_SKIP_COSIGN_CHECKSUM_VERIFY=1 to bypass (not recommended)." >&2
      return 1
    fi
  else
    actual="$(sha256_file "$destination" | tr '[:upper:]' '[:lower:]')"
    if [[ "$actual" != "$expected" ]]; then
      echo "Cosign checksum mismatch for ${asset} (${COSIGN_VERSION})." >&2
      echo "  expected: $expected" >&2
      echo "  actual:   $actual" >&2
      return 1
    fi
  fi

  echo "$destination"
}

verify_signature() {
  local cosign_bin="$1"
  local archive="$2"
  local signature="$3"
  local pubkey="$4"

  "$cosign_bin" verify-blob --key "$pubkey" --signature "$signature" "$archive" >/dev/null
}

main() {
  local requested_version target version asset_name base_url tmp_dir archive checksum signature pubkey cosign_bin

  requested_version="${FPS_TRACKER_VERSION:-${1:-}}"
  target="$(detect_target)"
  version="$(resolve_version "$requested_version")"

  asset_name="${BINARY_NAME}-${target}.tar.gz"
  if [[ -n "$BASE_URL_OVERRIDE" ]]; then
    base_url="${BASE_URL_OVERRIDE%/}"
  else
    base_url="https://github.com/${REPO}/releases/download/${version}"
  fi

  tmp_dir="$(mktemp -d)"
  trap 'rm -rf "${tmp_dir:-}"' EXIT

  archive="${tmp_dir}/${asset_name}"
  checksum="${tmp_dir}/${asset_name}.sha256"
  signature="${tmp_dir}/${asset_name}.sig"
  pubkey="${tmp_dir}/cosign.pub"

  echo "Installing ${BINARY_NAME} ${version} (${target})"
  curl_download "${base_url}/${asset_name}" "$archive"
  curl_download "${base_url}/${asset_name}.sha256" "$checksum"

  if [[ "$SKIP_SIG_VERIFY" != "1" ]]; then
    if curl_download "${base_url}/${asset_name}.sig" "$signature"; then
      if [[ -n "$COSIGN_PUBKEY_OVERRIDE" ]]; then
        pubkey="$COSIGN_PUBKEY_OVERRIDE"
        if [[ ! -f "$pubkey" ]]; then
          echo "COSIGN pubkey override not found: $pubkey" >&2
          exit 1
        fi
      else
        if ! curl_download "${base_url}/cosign.pub" "$pubkey"; then
          if [[ "$REQUIRE_SIG_VERIFY" == "1" ]]; then
            echo "Signature assets not available for this release. Set FPS_TRACKER_SKIP_SIGNATURE_VERIFY=1 to bypass." >&2
            exit 1
          fi
          echo "Signature assets not available for this release; skipping signature verification." >&2
          pubkey=""
        fi
      fi
      if [[ -n "$pubkey" ]]; then
        cosign_bin="$(ensure_cosign "$tmp_dir")"
        verify_signature "$cosign_bin" "$archive" "$signature" "$pubkey"
        echo "Signature verification succeeded."
      fi
    else
      if [[ "$REQUIRE_SIG_VERIFY" == "1" ]]; then
        echo "Signature assets not available for this release. Set FPS_TRACKER_SKIP_SIGNATURE_VERIFY=1 to bypass." >&2
        exit 1
      fi
      echo "Signature assets not available for this release; skipping signature verification." >&2
    fi
  else
    echo "Signature verification skipped (FPS_TRACKER_SKIP_SIGNATURE_VERIFY=1)." >&2
  fi

  verify_checksum "$archive" "$checksum"

  mkdir -p "$INSTALL_DIR"
  tar -xzf "$archive" -C "$tmp_dir"
  install -m 755 "${tmp_dir}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"

  echo
  echo "Installed to: ${INSTALL_DIR}/${BINARY_NAME}"
  if [[ ":$PATH:" != *":${INSTALL_DIR}:"* ]]; then
    echo "Add this directory to PATH if needed: ${INSTALL_DIR}"
  fi
  echo "Run: ${BINARY_NAME} --help"
}

main "$@"
