#!/usr/bin/env bash
set -euo pipefail

VERSION_TAG="${1:-}"
DIST_DIR="${2:-dist}"
REPO="${FPS_TRACKER_REPO:-forgemypcgit/FPStracker}"

if [[ -z "${VERSION_TAG}" ]]; then
  echo "Usage: $0 <version-tag> [dist-dir]" >&2
  echo "Example: $0 v0.2.0 dist" >&2
  exit 1
fi

VERSION="${VERSION_TAG#v}"
RELEASE_BASE_URL="https://github.com/${REPO}/releases/download/${VERSION_TAG}"

sha_for_asset() {
  local asset="$1"
  local checksum_path
  checksum_path="$(find "${DIST_DIR}" -type f -name "${asset}.sha256" | head -n1 || true)"
  if [[ -z "${checksum_path}" ]]; then
    echo "Missing checksum file for asset: ${asset}" >&2
    exit 1
  fi
  awk '{print $1}' "${checksum_path}" | head -n1
}

LINUX_SHA="$(sha_for_asset "fps-tracker-x86_64-unknown-linux-gnu.tar.gz")"
MAC_X64_SHA="$(sha_for_asset "fps-tracker-x86_64-apple-darwin.tar.gz")"
MAC_ARM_SHA="$(sha_for_asset "fps-tracker-aarch64-apple-darwin.tar.gz")"
WIN_X64_SHA="$(sha_for_asset "fps-tracker-x86_64-pc-windows-msvc.zip" | tr '[:lower:]' '[:upper:]')"

HOMEBREW_DIR="${DIST_DIR}/package-managers/homebrew"
WINGET_DIR="${DIST_DIR}/package-managers/winget/manifests/f/ForgeMyPC/FPSTracker/${VERSION}"

mkdir -p "${HOMEBREW_DIR}" "${WINGET_DIR}"

cat > "${HOMEBREW_DIR}/fps-tracker.rb" <<EOF
class FpsTracker < Formula
  desc "FPS benchmark capture and submission CLI"
  homepage "https://github.com/${REPO}"
  version "${VERSION}"
  license "PolyForm-Noncommercial-1.0.0"

  on_macos do
    if Hardware::CPU.arm?
      url "${RELEASE_BASE_URL}/fps-tracker-aarch64-apple-darwin.tar.gz"
      sha256 "${MAC_ARM_SHA}"
    else
      url "${RELEASE_BASE_URL}/fps-tracker-x86_64-apple-darwin.tar.gz"
      sha256 "${MAC_X64_SHA}"
    end
  end

  on_linux do
    url "${RELEASE_BASE_URL}/fps-tracker-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "${LINUX_SHA}"
  end

  def install
    bin.install "fps-tracker"
  end

  test do
    assert_match "fps-tracker", shell_output("#{bin}/fps-tracker --help")
  end
end
EOF

cat > "${WINGET_DIR}/ForgeMyPC.FPSTracker.yaml" <<EOF
PackageIdentifier: ForgeMyPC.FPSTracker
PackageVersion: ${VERSION}
DefaultLocale: en-US
ManifestType: version
ManifestVersion: 1.6.0
EOF

cat > "${WINGET_DIR}/ForgeMyPC.FPSTracker.installer.yaml" <<EOF
PackageIdentifier: ForgeMyPC.FPSTracker
PackageVersion: ${VERSION}
MinimumOSVersion: 10.0.0.0
Installers:
  - Architecture: x64
    InstallerType: zip
    NestedInstallerType: portable
    NestedInstallerFiles:
      - RelativeFilePath: fps-tracker.exe
        PortableCommandAlias: fps-tracker
    InstallerUrl: ${RELEASE_BASE_URL}/fps-tracker-x86_64-pc-windows-msvc.zip
    InstallerSha256: ${WIN_X64_SHA}
ManifestType: installer
ManifestVersion: 1.6.0
EOF

cat > "${WINGET_DIR}/ForgeMyPC.FPSTracker.locale.en-US.yaml" <<EOF
PackageIdentifier: ForgeMyPC.FPSTracker
PackageVersion: ${VERSION}
PackageLocale: en-US
Publisher: ForgeMyPC
PublisherUrl: https://github.com/${REPO}
PublisherSupportUrl: https://github.com/${REPO}/issues
Author: ForgeMyPC
PackageName: FPS Tracker
PackageUrl: https://github.com/${REPO}
License: PolyForm-Noncommercial-1.0.0
LicenseUrl: https://github.com/${REPO}/blob/main/LICENSE
ShortDescription: FPS benchmark capture and submission CLI
Description: Collect and submit gaming benchmark data with hardware detection and live frame-time capture.
Moniker: fps-tracker
Tags:
  - benchmark
  - fps
  - gaming
ReleaseNotesUrl: https://github.com/${REPO}/releases/tag/${VERSION_TAG}
ManifestType: defaultLocale
ManifestVersion: 1.6.0
EOF

echo "Generated package manager manifests:"
echo "  Homebrew: ${HOMEBREW_DIR}/fps-tracker.rb"
echo "  winget:   ${WINGET_DIR}"
