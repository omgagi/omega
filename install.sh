#!/usr/bin/env bash
set -euo pipefail

REPO="omega-cortex/omega"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# ── Detect OS and architecture ───────────────────────────────────────────────

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Linux)  os="linux" ;;
    Darwin) os="darwin" ;;
    *)      echo "Error: unsupported OS: $OS"; exit 1 ;;
esac

case "$ARCH" in
    x86_64|amd64)   arch="x86_64" ;;
    aarch64|arm64)  arch="aarch64" ;;
    *)              echo "Error: unsupported architecture: $ARCH"; exit 1 ;;
esac

ASSET="omega-${os}-${arch}"

# ── Check available release for this platform ────────────────────────────────

echo ""
echo "  OMEGA Installer"
echo "  ─────────────────────────────────"
echo "  OS:   $OS ($os)"
echo "  Arch: $ARCH ($arch)"
echo ""

# Resolve latest release tag
if ! command -v curl &>/dev/null; then
    echo "Error: curl is required but not found."; exit 1
fi

LATEST_URL="https://api.github.com/repos/${REPO}/releases/latest"
TAG=$(curl -fsSL "$LATEST_URL" | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"//;s/".*//')

if [ -z "$TAG" ]; then
    echo "Error: could not find a release. Check https://github.com/${REPO}/releases"
    exit 1
fi

DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${TAG}/${ASSET}.tar.gz"

echo "  Release: $TAG"
echo "  Asset:   ${ASSET}.tar.gz"
echo ""

# ── Download and install ─────────────────────────────────────────────────────

TMPDIR_INSTALL="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_INSTALL"' EXIT

echo "  Downloading..."
if ! curl -fsSL "$DOWNLOAD_URL" -o "$TMPDIR_INSTALL/${ASSET}.tar.gz"; then
    echo ""
    echo "  Error: download failed."
    echo "  No pre-built binary available for ${os}/${arch} in release ${TAG}."
    echo ""
    echo "  Build from source instead:"
    echo "    git clone https://github.com/${REPO}"
    echo "    cd omega/backend && cargo build --release"
    echo ""
    exit 1
fi

echo "  Extracting..."
tar xzf "$TMPDIR_INSTALL/${ASSET}.tar.gz" -C "$TMPDIR_INSTALL"

# ── Install binary ───────────────────────────────────────────────────────────

mkdir -p "$INSTALL_DIR"
mv "$TMPDIR_INSTALL/$ASSET" "$INSTALL_DIR/omega"
chmod +x "$INSTALL_DIR/omega"

echo "  Installed to: $INSTALL_DIR/omega"

# ── Ensure PATH includes install dir ─────────────────────────────────────────

if ! echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
    echo ""
    echo "  Add to your shell profile:"
    echo "    export PATH=\"$INSTALL_DIR:\$PATH\""
    echo ""
fi

# ── Run init wizard ──────────────────────────────────────────────────────────

echo ""
echo "  ─────────────────────────────────"
echo "  Starting Omega setup..."
echo ""

"$INSTALL_DIR/omega" init
