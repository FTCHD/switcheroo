#!/bin/sh
# Switcheroo installer for macOS and Linux.
#
#   curl -fsSL https://raw.githubusercontent.com/ftchd/switcheroo/main/install.sh | sh
#
# Downloads the release binary for this machine, checks it against the release's SHA256SUMS,
# and puts it on your PATH. Because the download happens with curl, macOS does not attach the
# quarantine attribute that triggers Gatekeeper, so `switcheroo` runs without a prompt.
#
# Environment:
#   SWITCHEROO_VERSION      tag to install (default: latest release)
#   SWITCHEROO_INSTALL_DIR  where to put the binary (default: /usr/local/bin if writable, else ~/.local/bin)
#   SWITCHEROO_DRY_RUN=1    print what would happen and exit
set -eu

repo="ftchd/switcheroo"
version="${SWITCHEROO_VERSION:-latest}"

say() { printf '%s\n' "$*" >&2; }
fail() { say "install.sh: $*"; exit 1; }

os="$(uname -s)"
arch="$(uname -m)"
case "$os/$arch" in
    Darwin/arm64)                     asset="switcheroo-macos-aarch64" ;;
    Darwin/x86_64)                    asset="switcheroo-macos-x86_64" ;;
    Linux/x86_64 | Linux/amd64)       asset="switcheroo-linux-x86_64" ;;
    Linux/aarch64 | Linux/arm64)      asset="switcheroo-linux-aarch64" ;;
    *) fail "no prebuilt binary for $os/$arch; build from source: https://github.com/$repo#install" ;;
esac

if [ "$version" = "latest" ]; then
    base="https://github.com/$repo/releases/latest/download"
else
    base="https://github.com/$repo/releases/download/$version"
fi

if [ -n "${SWITCHEROO_INSTALL_DIR:-}" ]; then
    dir="$SWITCHEROO_INSTALL_DIR"
elif [ -d /usr/local/bin ] && [ -w /usr/local/bin ]; then
    dir="/usr/local/bin"
else
    dir="$HOME/.local/bin"
fi

if [ "${SWITCHEROO_DRY_RUN:-0}" = "1" ]; then
    say "would download $base/$asset and $base/SHA256SUMS"
    say "would install to $dir/switcheroo"
    exit 0
fi

command -v curl >/dev/null 2>&1 || fail "curl is required"

tmp="$(mktemp -d 2>/dev/null || mktemp -d -t switcheroo)"
trap 'rm -rf "$tmp"' EXIT

say "Downloading $asset ($version)…"
curl -fsSL "$base/$asset" -o "$tmp/switcheroo" || fail "download failed: $base/$asset"
curl -fsSL "$base/SHA256SUMS" -o "$tmp/SHA256SUMS" || fail "download failed: $base/SHA256SUMS"

expected="$(grep " $asset\$" "$tmp/SHA256SUMS" | awk '{print $1}')"
[ -n "$expected" ] || fail "no checksum for $asset in SHA256SUMS"
if command -v sha256sum >/dev/null 2>&1; then
    actual="$(sha256sum "$tmp/switcheroo" | awk '{print $1}')"
else
    actual="$(shasum -a 256 "$tmp/switcheroo" | awk '{print $1}')"
fi
[ "$expected" = "$actual" ] || fail "checksum mismatch for $asset (expected $expected, got $actual)"

chmod +x "$tmp/switcheroo"
# Belt and braces: curl does not set this, but a download manager or a copy from a browser might.
if [ "$os" = "Darwin" ] && command -v xattr >/dev/null 2>&1; then
    xattr -d com.apple.quarantine "$tmp/switcheroo" 2>/dev/null || true
fi

mkdir -p "$dir"
mv "$tmp/switcheroo" "$dir/switcheroo"
say "Installed $("$dir/switcheroo" --version) to $dir/switcheroo"

case ":$PATH:" in
    *":$dir:"*) ;;
    *) say "Note: $dir is not on your PATH. Add it, for example:"
       say "  echo 'export PATH=\"$dir:\$PATH\"' >> ~/.zshrc" ;;
esac
say "Run \`switcheroo doctor\` to see what it found."
