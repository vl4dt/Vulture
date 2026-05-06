#!/usr/bin/env bash
# Vulturius Build Script
# Full workspace build with release packaging
#
# Usage: ./build.sh [target] [options]
#   targets: all, linux, windows, macos-x86, macos-arm, cli-only, gui-only
#   options: --debug, --bench, --docs, --skip-gui

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
BUILD_GUI=true

# Parse options
for arg in "$@"; do
    case $arg in
        --debug) PROFILE="debug" ;;
        --bench) PROFILE="bench" ;;
        --docs) PROFILE="doc" ;;
        --skip-gui) BUILD_GUI=false ;;
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

    if [ "$BUILD_GUI" = true ]; then
        command -v npm >/dev/null 2>&1 || { err "npm/Node.js not found (required for GUI)"; exit 1; }
        command -v tauri >/dev/null 2>&1 || { err "Tauri CLI not found. Install with: cargo install tauri-cli --version ^2"; exit 1; }
        tauri --version
    fi

    mkdir -p "$DIST_DIR"
}

# Build the frontend (Svelte + Vite)
build_frontend() {
    log "Building frontend..."
    local frontend_dir="${PROJECT_DIR}/crates/vulturius-gui/frontend"

    cd "$frontend_dir" || exit 1
    if [ ! -d "node_modules" ]; then
        log "Installing npm dependencies..."
        npm install
    fi
    npm run build
    cd "$PROJECT_DIR"
    log "Frontend built"
}

# Build the CLI (core + cli crates)
build_cli() {
    local target=$1
    local name=$2
    local ext="${3:-}"

    log "Building CLI for ${name} (${target})..."

    cargo build --${PROFILE} --target "$target" -p vulturius-core -p vulturius-cli

    local bin_name="vulturius-cli${ext}"
    local src="target/${target}/${PROFILE}/${bin_name}"

    if [ ! -f "$src" ]; then
        err "CLI binary not found: $src"
        return 1
    fi

    # Create dist directory
    local dist_subdir="${DIST_DIR}/vulturius-cli-${name}"
    mkdir -p "$dist_subdir"

    # Copy binary
    cp "$src" "$dist_subdir/"

    # Copy docs
    cp LICENSE "$dist_subdir/" 2>/dev/null || true
    cp README.md "$dist_subdir/" 2>/dev/null || true

    # Create archive
    if [[ "$name" == windows* ]]; then
        cd "$DIST_DIR" || exit 1
        if command -v zip >/dev/null 2>&1; then
            zip -r "vulturius-cli-${name}.zip" "vulturius-cli-${name}"
        else
            warn "zip not found, skipping archive creation"
        fi
        cd "$PROJECT_DIR" || exit 1
    else
        tar czf "${DIST_DIR}/vulturius-cli-${name}.tar.gz" -C "$DIST_DIR" "vulturius-cli-${name}"
    fi

    log "Built CLI: vulturius-cli-${name}"
}

# Build the GUI (Tauri)
build_gui() {
    local target=$1
    local name=$2

    log "Building GUI for ${name} (${target})..."

    local gui_dir="${PROJECT_DIR}/crates/vulturius-gui"
    cd "$gui_dir" || exit 1

    export TAURI_CI=true
    # Build with Tauri (release is default, --debug for debug builds)
    tauri_args=("build")
    if [ -n "$target" ]; then tauri_args+=("--target" "$target"); fi
    if [ "$PROFILE" = "debug" ]; then tauri_args+=("--debug"); fi

    tauri "${tauri_args[@]}"

    cd "$PROJECT_DIR" || exit 1

    # Copy Tauri output to dist
    local tauri_bundle_dir="${PROJECT_DIR}/target/${PROFILE}/bundle"

    if [ -d "$tauri_bundle_dir" ]; then
        log "Copying GUI artifacts to dist..."
        find "$tauri_bundle_dir" -type f \( -name "*.exe" -o -name "*.msi" -o -name "*.nsis" -o -name "*.deb" -o -name "*.AppImage" -o -name "*.dmg" -o -name "*.pkg" \) | while read -r file; do
            local ext=".${file##*.}"
            local out_name="vulturius-gui-${name}${ext}"
            cp "$file" "${DIST_DIR}/${out_name}"
            log "Copied: ${out_name}"
        done
    else
        warn "No Tauri bundle directory found"
    fi

    log "Built GUI: vulturius-gui-${name}"
}

# Build for a specific platform (CLI + GUI)
build_platform() {
    local target=$1
    local name=$2
    local ext="${3:-}"

    # Build CLI
    build_cli "$target" "$name" "$ext"

    # Build GUI if enabled
    if [ "$BUILD_GUI" = true ]; then
        if [ "${FRONTEND_BUILT:-false}" = false ]; then
            build_frontend
            FRONTEND_BUILT=true
        fi
        build_gui "$target" "$name"
    fi
}

# Main
FRONTEND_BUILT=false

if [ "$PROFILE" = "doc" ]; then
    preflight
    cargo doc --workspace --no-deps
    log "Documentation built in target/doc/"
    exit 0
fi

case $TARGET in
    all)
        preflight
        build_platform "x86_64-unknown-linux-gnu" "linux-x86_64" ""
        build_platform "x86_64-pc-windows-msvc" "windows-x86_64" ".exe"
        build_platform "x86_64-apple-darwin" "macos-x86_64" ""
        build_platform "aarch64-apple-darwin" "macos-aarch64" ""
        ;;
    linux)
        preflight
        build_platform "x86_64-unknown-linux-gnu" "linux-x86_64" ""
        ;;
    windows)
        preflight
        build_platform "x86_64-pc-windows-msvc" "windows-x86_64" ".exe"
        ;;
    macos-x86)
        preflight
        build_platform "x86_64-apple-darwin" "macos-x86_64" ""
        ;;
    macos-arm)
        preflight
        build_platform "aarch64-apple-darwin" "macos-aarch64" ""
        ;;
    cli-only)
        preflight
        cargo build --${PROFILE} -p vulturius-core -p vulturius-cli
        log "CLI built (no packaging)"
        ;;
    gui-only)
        preflight
        BUILD_GUI=true
        build_frontend
        FRONTEND_BUILT=true
        build_gui "x86_64-pc-windows-msvc" "windows-x86_64"
        ;;
    *)
        err "Unknown target: $TARGET"
        echo "Available targets: all, linux, windows, macos-x86, macos-arm, cli-only, gui-only"
        exit 1
        ;;
esac

log "Build complete! Artifacts in ${DIST_DIR}/"
ls -la "$DIST_DIR"/
