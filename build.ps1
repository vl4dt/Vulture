# Vulturius Build Script (PowerShell)
# Full workspace build with release packaging
#
# Usage: .\build.ps1 [target] [-Debug] [-Bench] [-Docs] [-SkipGUI]
#   targets: all, linux, windows, macos-x86, macos-arm, cli-only, gui-only
#   options: -Debug, -Bench, -Docs, -SkipGUI

param(
    [string]$Target = "all",
    [switch]$Debug,
    [switch]$Bench,
    [switch]$Docs,
    [switch]$SkipGUI
)

$ErrorActionPreference = "Stop"

# Config
$ProjectDir = $PSScriptRoot
$DistDir = Join-Path $ProjectDir "dist"
$Profile = "release"
$BuildGUI = -not $SkipGUI

if ($Debug) { $Profile = "debug" }
if ($Bench) { $Profile = "bench" }
if ($Docs) { $Profile = "doc" }

# Helpers
function Log { Write-Host "[BUILD] $args" -ForegroundColor Green }
function Warn { Write-Warning "[WARN] $args" }
function Err { Write-Error "[ERROR] $args"; exit 1 }

# Pre-flight checks
function Preflight {
    Log "Checking prerequisites..."
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Err "Rust/Cargo not found. Install from https://rustup.rs/"
    }
    rustc --version
    cargo --version

    if ($BuildGUI) {
        if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
            Err "npm/Node.js not found (required for GUI build). Install from https://nodejs.org/"
        }
        if (-not (Get-Command tauri -ErrorAction SilentlyContinue)) {
            Err "Tauri CLI not found. Install with: cargo install tauri-cli --version ^2"
        }
        tauri --version
    }

    if (-not (Test-Path $DistDir)) {
        New-Item -ItemType Directory -Path $DistDir | Out-Null
    }
}

# Build the frontend (Svelte + Vite)
function Build-Frontend {
    Log "Building frontend..."
    $frontendDir = Join-Path $ProjectDir "crates\vulturius-gui\frontend"

    Push-Location $frontendDir
    try {
        # Install dependencies
        if (-not (Test-Path (Join-Path $frontendDir "node_modules"))) {
            Log "Installing npm dependencies..."
            if (-not (npm ci)) { npm install }
        }

        # Build frontend
        Log "Running vite build..."
        npm run build
    }
    finally {
        Pop-Location
    }

    Log "Frontend built"
}

# Build the CLI (core + cli crates)
function Build-CLI {
    param(
        [string]$RustTarget,
        [string]$Name,
        [string]$Ext = ""
    )

    Log "Building CLI for ${Name} (${RustTarget})..."

    # Build core + cli
    cargo build --${Profile} --target $RustTarget -p vulturius-core -p vulturius-cli

    $rustTargetDir = "target\$RustTarget\$Profile"
    $binName = "vulturius-cli${Ext}"
    $srcBin = Join-Path $ProjectDir (Join-Path $rustTargetDir $binName)

    if (-not (Test-Path $srcBin)) {
        Err "CLI binary not found: $srcBin"
    }

    # Create dist directory
    $distSubdir = Join-Path $DistDir "vulturius-cli-${Name}"
    New-Item -ItemType Directory -Force -Path $distSubdir | Out-Null

    # Copy binary
    Copy-Item $srcBin $distSubdir -Force

    # Copy docs
    $licensePath = Join-Path $ProjectDir "LICENSE"
    $readmePath = Join-Path $ProjectDir "README.md"

    if (Test-Path $licensePath) { Copy-Item $licensePath $distSubdir -Force }
    if (Test-Path $readmePath) { Copy-Item $readmePath $distSubdir -Force }

    # Create archive
    $zipPath = Join-Path $DistDir "vulturius-cli-${Name}.zip"
    Compress-Archive -Path $distSubdir -DestinationPath $zipPath -Force

    Log "Built CLI: vulturius-cli-${Name}"
}

# Build the GUI (Tauri)
function Build-GUI {
    param(
        [string]$RustTarget,
        [string]$Name
    )

    Log "Building GUI for ${Name}..."

    $guiDir = Join-Path $ProjectDir "crates\vulturius-gui"

    Push-Location $guiDir
    try {
        # Build with Tauri (release is default, --debug for debug builds)
        # Note: skip --target for native builds (Tauri auto-detects host)
        $tauriArgs = @("build")
        if ($Profile -eq "debug") { $tauriArgs += "--debug" }

        & tauri $tauriArgs 2>&1
    }
    finally {
        Pop-Location
    }

    # Copy Tauri output to dist
    # Tauri outputs to target/release/bundle/ (workspace root)
    $tauriBundleDir = Join-Path $ProjectDir "target\release\bundle"

    if (Test-Path $tauriBundleDir) {
        Log "Copying GUI artifacts to dist..."
        Get-ChildItem -Path $tauriBundleDir -Recurse -Include "*.exe","*.msi","*.nsis","*.deb","*.AppImage","*.dmg","*.pkg","*.app" -ErrorAction SilentlyContinue | ForEach-Object {
            $outName = "vulturius-gui-${Name}$($_.Extension)"
            Copy-Item $_.FullName (Join-Path $DistDir $outName) -Force
            Log "Copied: $outName"
        }
    } else {
        Warn "No Tauri bundle directory found at $tauriBundleDir"
    }

    Log "Built GUI: vulturius-gui-${Name}"
}

# Build for a specific platform (CLI + GUI)
function Build-Platform {
    param(
        [string]$RustTarget,
        [string]$Name,
        [string]$Ext = ""
    )

    # Build CLI
    Build-CLI -RustTarget $RustTarget -Name $Name -Ext $Ext

    # Build GUI if enabled
    if ($BuildGUI) {
        # Build frontend first (only once)
        if (-not $script:FrontendBuilt) {
            Build-Frontend
            $script:FrontendBuilt = $true
        }
        Build-GUI -RustTarget $RustTarget -Name $Name
    }
}

# Main
$script:FrontendBuilt = $false

if ($Profile -eq "doc") {
    Preflight
    cargo doc --workspace --no-deps
    Log "Documentation built in target/doc/"
    exit 0
}

switch ($Target) {
    "all" {
        Preflight
        Build-Platform -RustTarget "x86_64-unknown-linux-gnu" -Name "linux-x86_64" -Ext ""
        Build-Platform -RustTarget "x86_64-pc-windows-msvc" -Name "windows-x86_64" -Ext ".exe"
        Build-Platform -RustTarget "x86_64-apple-darwin" -Name "macos-x86_64" -Ext ""
        Build-Platform -RustTarget "aarch64-apple-darwin" -Name "macos-aarch64" -Ext ""
    }
    "linux" {
        Preflight
        Build-Platform -RustTarget "x86_64-unknown-linux-gnu" -Name "linux-x86_64" -Ext ""
    }
    "windows" {
        Preflight
        Build-Platform -RustTarget "x86_64-pc-windows-msvc" -Name "windows-x86_64" -Ext ".exe"
    }
    "macos-x86" {
        Preflight
        Build-Platform -RustTarget "x86_64-apple-darwin" -Name "macos-x86_64" -Ext ""
    }
    "macos-arm" {
        Preflight
        Build-Platform -RustTarget "aarch64-apple-darwin" -Name "macos-aarch64" -Ext ""
    }
    "cli-only" {
        Preflight
        cargo build --${Profile} -p vulturius-core -p vulturius-cli
        Log "CLI built (no packaging)"
    }
    "gui-only" {
        Preflight
        $BuildGUI = $true
        Build-Frontend
        $script:FrontendBuilt = $true
        Build-GUI -RustTarget "x86_64-pc-windows-msvc" -Name "windows-x86_64"
    }
    default {
        Err "Unknown target: $Target"
        Write-Host "Available targets: all, linux, windows, macos-x86, macos-arm, cli-only, gui-only"
        exit 1
    }
}

Log "Build complete! Artifacts in $DistDir"
Get-ChildItem $DistDir -File | Format-Table Name, Length
