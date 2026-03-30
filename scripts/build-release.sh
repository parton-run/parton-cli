#!/usr/bin/env bash
set -euo pipefail

# Build release binaries for Parton CLI.
# Usage:
#   ./scripts/build-release.sh              # build all targets
#   ./scripts/build-release.sh macos        # macOS only (arm64 + x86_64)
#   ./scripts/build-release.sh macos-arm64  # single target

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
        bash -c "cargo build --release --target x86_64-unknown-linux-gnu"

    cp "target/x86_64-unknown-linux-gnu/release/${BIN_NAME}" "${RELEASE_DIR}/${BIN_NAME}-linux-x64"
    chmod +x "${RELEASE_DIR}/${BIN_NAME}-linux-x64"
    echo "  ✔ ${BIN_NAME}-linux-x64"
}

case "${1:-all}" in
    all)
        build_macos_arm64
        build_macos_x86
        build_linux_x86
        ;;
    macos)
        build_macos_arm64
        build_macos_x86
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
