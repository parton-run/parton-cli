#!/usr/bin/env bash
set -euo pipefail

# Build release binaries + install script for Parton CLI.
# Usage:
#   ./scripts/build-release.sh              # build all targets
#   ./scripts/build-release.sh macos        # macOS only (arm64 + x86_64)
#   ./scripts/build-release.sh macos-arm64  # single target
#   ./scripts/build-release.sh linux        # Linux x86_64 via Docker

VERSION="${VERSION:-$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')}"
RELEASE_DIR="release/v${VERSION}"
BIN_NAME="parton"

echo "Building Parton v${VERSION}"
echo "Output: ${RELEASE_DIR}/"
echo ""

mkdir -p "$RELEASE_DIR"

build_macos_arm64() {
    echo "→ Building aarch64-apple-darwin..."
    cargo build --release --target aarch64-apple-darwin
    cp "target/aarch64-apple-darwin/release/${BIN_NAME}" "${RELEASE_DIR}/${BIN_NAME}-darwin-arm64"
    chmod +x "${RELEASE_DIR}/${BIN_NAME}-darwin-arm64"
    echo "  ✔ ${BIN_NAME}-darwin-arm64"
}

build_macos_x86() {
    echo "→ Building x86_64-apple-darwin..."
    cargo build --release --target x86_64-apple-darwin
    cp "target/x86_64-apple-darwin/release/${BIN_NAME}" "${RELEASE_DIR}/${BIN_NAME}-darwin-x64"
    chmod +x "${RELEASE_DIR}/${BIN_NAME}-darwin-x64"
    echo "  ✔ ${BIN_NAME}-darwin-x64"
}

build_linux_x86() {
    echo "→ Building x86_64-unknown-linux-gnu (via Docker)..."

    if ! command -v docker &>/dev/null; then
        echo "  ✗ Docker not found — skipping Linux build"
        return 1
    fi

    if ! docker info &>/dev/null 2>&1; then
        echo "  ✗ Docker daemon not running — skipping Linux build"
        return 1
    fi

    docker run --rm \
        --platform linux/amd64 \
        -v "$(pwd)":/app \
        -w /app \
        rust:latest \
        bash -c "apt-get update -qq && apt-get install -y -qq cmake >/dev/null 2>&1 && cargo build --release --target x86_64-unknown-linux-gnu"

    cp "target/x86_64-unknown-linux-gnu/release/${BIN_NAME}" "${RELEASE_DIR}/${BIN_NAME}-linux-x64"
    chmod +x "${RELEASE_DIR}/${BIN_NAME}-linux-x64"
    echo "  ✔ ${BIN_NAME}-linux-x64"
}

generate_checksums() {
    echo "→ Generating checksums..."
    cd "$RELEASE_DIR"
    shasum -a 256 parton-* > checksums.txt 2>/dev/null || sha256sum parton-* > checksums.txt 2>/dev/null || true
    cd - > /dev/null
    if [ -f "${RELEASE_DIR}/checksums.txt" ]; then
        echo "  ✔ checksums.txt"
    fi
}

generate_install_script() {
    echo "→ Generating install.sh..."

    cat > "${RELEASE_DIR}/install.sh" << 'INSTALLER'
#!/usr/bin/env bash
set -euo pipefail

# Parton installer
# Usage: curl -fsSL https://parton.run/install.sh | sh
#   or:  PARTON_VERSION=v0.3.3 curl -fsSL https://parton.run/install.sh | sh

REPO="parton-run/parton-cli"
VERSION="${PARTON_VERSION:-latest}"

if [ "$VERSION" = "latest" ]; then
  BASE_URL="https://github.com/${REPO}/releases/latest/download"
else
  BASE_URL="https://github.com/${REPO}/releases/download/${VERSION}"
fi

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
DOWNLOAD_URL="${BASE_URL}/${BINARY_NAME}"

TMP_DIR="$(mktemp -d)"
TMP_BIN="${TMP_DIR}/parton"

echo "Installing Parton ${VERSION} for ${PLATFORM}-${ARCH_NAME}..."
curl -fSL "$DOWNLOAD_URL" -o "$TMP_BIN"
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
INSTALLER

    chmod +x "${RELEASE_DIR}/install.sh"
    echo "  ✔ install.sh (downloads from GitHub releases, default: latest)"
}

case "${1:-all}" in
    all)
        build_macos_arm64
        build_macos_x86
        build_linux_x86
        generate_checksums
        generate_install_script
        ;;
    macos)
        build_macos_arm64
        build_macos_x86
        generate_checksums
        generate_install_script
        ;;
    macos-arm64)
        build_macos_arm64
        ;;
    macos-x86)
        build_macos_x86
        ;;
    linux)
        build_linux_x86
        ;;
    *)
        echo "Usage: $0 [all|macos|macos-arm64|macos-x86|linux]"
        exit 1
        ;;
esac

echo ""
echo "Done. Release artifacts:"
ls -lh "$RELEASE_DIR"/ 2>/dev/null || echo "  (none)"
