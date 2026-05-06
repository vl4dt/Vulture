#!/usr/bin/env bash
# Vulturius Build Script
# Cross-platform build with release packaging
#
# Usage: ./build.sh [target] [options]
#   targets: all, linux, windows, macos-x86, macos-arm, cli-only
#   options: --debug, --bench, --docs

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Config
PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)"
DIST_DIR="${PROJECT_DIR}/dist"
PROFILE="release"
TARGET="${1:-all}"

# Parse options
for arg in "$@"; do
    case $arg in
        --debug) PROFILE="debug" ;;
        --bench) PROFILE="bench" ;;
        --docs) PROFILE="doc" ;;
    esac
done

log() { echo -e "${GREEN}[BUILD]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
err() { echo -e "${RED}[ERROR]${NC} $*" >&2; }

# Pre-flight checks
preflight() {
    log "Checking prerequisites..."
    command -v cargo >/dev/null 2>&1 || { err "Rust/Cargo not found"; exit 1; }
    rustc --version
    cargo --version
    mkdir -p "$DIST_DIR"
}

# Build for a specific target
build_target() {
    local target=$1
    local name=$2
    local ext="${3:-}"

    log "Building for ${name} (${target})..."

    if [ "$PROFILE" = "doc" ]; then
        cargo doc --workspace --no-deps
        log "Documentation built in target/doc/"
        return
    fi

    cargo build --${PROFILE} --target "$target" -p vulturius-core -p vulturius-cli

    local bin_name="vulturius-cli${ext}"
    local src="target/${target}/${PROFILE}/${bin_name}"

    if [ ! -f "$src" ]; then
        err "Binary not found: $src"
        return 1
    fi

    # Create dist directory
    local dist_subdir="${DIST_DIR}/vulturius-${name}"
    mkdir -p "$dist_subdir"

    # Copy binary
    cp "$src" "$dist_subdir/"

    # Strip (Linux only)
    if [[ "$name" == linux* ]]; then
        strip "$dist_subdir/$bin_name" 2>/dev/null || true
    fi

    # Copy docs
    cp LICENSE "$dist_subdir/" 2>/dev/null || true
    cp README.md "$dist_subdir/" 2>/dev/null || true

    # Create archive
    if [[ "$name" == windows* ]]; then
        # Windows: use zip
        cd "$DIST_DIR"
        if command -v zip >/dev/null 2>&1; then
            zip -r "vulturius-${name}.zip" "vulturius-${name}"
        else
            warn "zip not found, skipping archive creation"
        fi
        cd "$PROJECT_DIR"
    else
        tar czf "${DIST_DIR}/vulturius-${name}.tar.gz" -C "$DIST_DIR" "vulturius-${name}"
    fi

    log "Built: vulturius-${name}"
}

# Main build
case $TARGET in
    all)
        preflight
        build_target "x86_64-unknown-linux-gnu" "linux-x86_64" ""
        build_target "x86_64-pc-windows-msvc" "windows-x86_64" ".exe"
        build_target "x86_64-apple-darwin" "macos-x86_64" ""
        build_target "aarch64-apple-darwin" "macos-aarch64" ""
        ;;
    linux)
        preflight
        build_target "x86_64-unknown-linux-gnu" "linux-x86_64" ""
        ;;
    windows)
        preflight
        build_target "x86_64-pc-windows-msvc" "windows-x86_64" ".exe"
        ;;
    macos-x86)
        preflight
        build_target "x86_64-apple-darwin" "macos-x86_64" ""
        ;;
    macos-arm)
        preflight
        build_target "aarch64-apple-darwin" "macos-aarch64" ""
        ;;
    cli-only)
        preflight
        cargo build --${PROFILE} -p vulturius-core -p vulturius-cli
        log "CLI built (no packaging)"
        ;;
    *)
        err "Unknown target: $TARGET"
        echo "Available targets: all, linux, windows, macos-x86, macos-arm, cli-only"
        exit 1
        ;;
esac

log "Build complete! Artifacts in ${DIST_DIR}/"
ls -la "$DIST_DIR"/
