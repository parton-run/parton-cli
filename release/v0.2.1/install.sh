#!/usr/bin/env bash
set -euo pipefail

# Parton installer
# Usage: curl -fsSL https://parton.run/install.sh | sh

VERSION="${PARTON_VERSION:-v0.2.1}"
BASE_URL="https://cdn.parton.run"

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Darwin) PLATFORM="darwin" ;;
  Linux)  PLATFORM="linux" ;;
  *)
    echo "Unsupported OS: $OS"
    exit 1
    ;;
esac

case "$ARCH" in
  arm64|aarch64) ARCH_NAME="arm64" ;;
  x86_64)        ARCH_NAME="x64" ;;
  *)
    echo "Unsupported architecture: $ARCH"
    exit 1
    ;;
esac

BINARY_NAME="parton-${PLATFORM}-${ARCH_NAME}"
DOWNLOAD_URL="${BASE_URL}/${VERSION}/${BINARY_NAME}"

TMP_DIR="$(mktemp -d)"
TMP_BIN="${TMP_DIR}/parton"

echo "Installing Parton ${VERSION} for ${PLATFORM}-${ARCH_NAME}..."
curl -fL "$DOWNLOAD_URL" -o "$TMP_BIN"
chmod +x "$TMP_BIN"

if [ -d "$HOME/.local/bin" ]; then
  INSTALL_DIR="$HOME/.local/bin"
else
  INSTALL_DIR="/usr/local/bin"
fi

mkdir -p "$INSTALL_DIR"

if [ -w "$INSTALL_DIR" ]; then
  mv "$TMP_BIN" "$INSTALL_DIR/parton"
else
  echo "Need elevated permissions to install to $INSTALL_DIR"
  sudo mv "$TMP_BIN" "$INSTALL_DIR/parton"
fi

rm -rf "$TMP_DIR"

echo ""
echo "Parton installed to: $INSTALL_DIR/parton"
echo ""

if command -v parton >/dev/null 2>&1; then
  parton version
  echo ""
  echo "Run: parton run \"your task here\""
else
  echo "Make sure $INSTALL_DIR is in your PATH:"
  echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
fi
