#!/usr/bin/env bash
#
# Regenerate the Homebrew formula (CLI) and cask (app) for a Burd release,
# writing them into an already-checked-out copy of the tap repo.
#
# Called by the release workflow after the GitHub release assets are published,
# but runnable by hand too.
#
# Usage:
#   VERSION=1.8.0 REPO=digitalnodecom/burd scripts/update-homebrew-tap.sh <tap-dir>
#
# Env:
#   VERSION  release version without the leading 'v' (required)
#   REPO     owner/name of the burd repo (default: digitalnodecom/burd)
#
set -euo pipefail

TAP_DIR="${1:?usage: update-homebrew-tap.sh <tap-dir>}"
VERSION="${VERSION:?VERSION is required}"
REPO="${REPO:-digitalnodecom/burd}"
BASE="https://github.com/$REPO/releases/download/v$VERSION"

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum | cut -d' ' -f1
  else shasum -a 256 | cut -d' ' -f1; fi
}

echo "Fetching checksums for v$VERSION..."

# CLI binaries publish a .sha256 sidecar; read it directly.
CLI_ARM="$(curl -fsSL "$BASE/burd-darwin-aarch64.sha256")"
CLI_X64="$(curl -fsSL "$BASE/burd-darwin-x64.sha256")"

# DMGs are signed/notarized (non-deterministic), so hash the actual assets.
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
curl -fsSL "$BASE/Burd_${VERSION}_aarch64.dmg" -o "$tmp/arm.dmg"
curl -fsSL "$BASE/Burd_${VERSION}_x64.dmg" -o "$tmp/x64.dmg"
DMG_ARM="$(sha256 < "$tmp/arm.dmg")"
DMG_X64="$(sha256 < "$tmp/x64.dmg")"

for v in CLI_ARM CLI_X64 DMG_ARM DMG_X64; do
  [ -n "${!v}" ] || { echo "error: empty checksum $v" >&2; exit 1; }
done

mkdir -p "$TAP_DIR/Formula" "$TAP_DIR/Casks"

# NOTE: `#{version}` below is Ruby string interpolation, left literal on purpose
# (the heredoc is unquoted so only $shell vars expand).
cat > "$TAP_DIR/Formula/burd.rb" <<EOF
class Burd < Formula
  desc "Local development environment manager for macOS (CLI)"
  homepage "https://github.com/digitalnodecom/burd"
  version "${VERSION}"
  license "PolyForm-Noncommercial-1.0.0"

  on_macos do
    on_arm do
      url "https://github.com/digitalnodecom/burd/releases/download/v#{version}/burd-darwin-aarch64"
      sha256 "${CLI_ARM}"
    end
    on_intel do
      url "https://github.com/digitalnodecom/burd/releases/download/v#{version}/burd-darwin-x64"
      sha256 "${CLI_X64}"
    end
  end

  def install
    if Hardware::CPU.arm?
      bin.install "burd-darwin-aarch64" => "burd"
    else
      bin.install "burd-darwin-x64" => "burd"
    end
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/burd --version")
  end
end
EOF

cat > "$TAP_DIR/Casks/burd.rb" <<EOF
cask "burd" do
  version "${VERSION}"

  on_arm do
    sha256 "${DMG_ARM}"

    url "https://github.com/digitalnodecom/burd/releases/download/v#{version}/Burd_#{version}_aarch64.dmg"
  end
  on_intel do
    sha256 "${DMG_X64}"

    url "https://github.com/digitalnodecom/burd/releases/download/v#{version}/Burd_#{version}_x64.dmg"
  end

  name "Burd"
  desc "Local development environment manager"
  homepage "https://github.com/digitalnodecom/burd"

  depends_on macos: :big_sur

  app "Burd.app"

  zap trash: [
    "~/Library/Application Support/Burd",
    "~/Library/Logs/Burd",
  ]
end
EOF

echo "Wrote $TAP_DIR/Formula/burd.rb and $TAP_DIR/Casks/burd.rb for v$VERSION"
