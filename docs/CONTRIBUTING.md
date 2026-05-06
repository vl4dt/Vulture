# Contributing to Vulturius

## Getting Started

### Prerequisites
- **Rust 1.85+** (stable toolchain)
- **Cargo** (bundled with Rust)
- **Node.js 18+** (for GUI development)
- **Git**

### Setup
```bash
git clone https://github.com/vulturius/vulturius.git
cd vulturius
cargo build --workspace
```

### Run Tests
```bash
cargo test --workspace
```

### Run Benchmarks
```bash
cargo bench -p vulturius-core
```

## Project Structure

```
Vulture/
├── crates/
│   ├── vulturius-core/     # Core library (all business logic)
│   ├── vulturius-cli/      # CLI application
│   └── vulturius-gui/      # Tauri 2 desktop GUI
├── build.sh                # Bash build script (Linux/macOS)
├── build.ps1               # PowerShell build script (Windows)
├── .github/workflows/      # CI/CD pipelines
└── docs/                   # Documentation
```

## Building for Distribution

The project includes build scripts for packaging release artifacts across platforms.
Both the **CLI** and **GUI** (Tauri desktop app) are built by default.

### Prerequisites (GUI builds)
- **Node.js 18+** and **npm** (for the Svelte frontend)
- **Tauri CLI 2**: `cargo install tauri-cli --version ^2`

### Linux / macOS (Bash)
```bash
# Build for a specific platform (CLI + GUI)
./build.sh windows
./build.sh linux
./build.sh macos-x86
./build.sh macos-arm

# Build for all platforms (requires cross-compilation targets)
./build.sh all

# Build only the CLI (skip GUI)
./build.sh windows --skip-gui
./build.sh cli-only

# Build only the GUI
./build.sh gui-only

# Debug build
./build.sh windows --debug
```

### Windows (PowerShell)
```powershell
# Build for Windows (CLI + GUI)
.\build.ps1 windows

# Build for all platforms
.\build.ps1 all

# Build only the CLI (skip GUI)
.\build.ps1 windows -SkipGUI
.\build.ps1 cli-only

# Build only the GUI
.\build.ps1 gui-only

# Debug build
.\build.ps1 windows -Debug
```

### Output
Both scripts produce packaged artifacts in the `dist/` directory:
```
dist/
├── vulturius-cli-windows-x86_64/
│   ├── vulturius-cli.exe
│   ├── LICENSE
│   └── README.md
├── vulturius-cli-windows-x86_64.zip
├── vulturius-gui-windows-x86_64.exe    # NSIS installer
├── vulturius-cli-linux-x86_64/
│   └── vulturius-cli
├── vulturius-cli-linux-x86_64.tar.gz
└── ...
```

### Cross-Compilation Setup
For `all` target builds, install the required Rust targets:
```bash
rustup target add x86_64-unknown-linux-gnu
rustup target add x86_64-pc-windows-msvc
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin
```

## Development Workflow

### 1. Fork & Branch
```bash
git checkout -b feature/my-feature
```

### 2. Implement
- Follow Rust naming conventions (`snake_case` for functions/vars, `PascalCase` for types)
- Add doc comments for public APIs
- Use `tracing` for logging (not `println!`)
- Add error handling with `VulturiusError` variants

### 3. Test
- Add unit tests in `#[cfg(test)] mod tests` blocks
- Test all branches of logic
- Use `tempfile` for filesystem tests
- Use `#[tokio::test]` for async tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_my_feature() {
        // ...
    }

    #[tokio::test]
    async fn test_async_feature() {
        // ...
    }
}
```

### 4. Lint & Format
```bash
cargo fmt --all
cargo clippy --workspace -- -D warnings
```

### 5. Commit
Follow [Conventional Commits](https://www.conventionalcommits.org/):
```
feat: add new scanner optimization
fix: resolve path traversal vulnerability
docs: update API reference
test: add incremental scan tests
perf: optimize database queries
security: add path sandboxing
```

### 6. Push & PR
```bash
git push origin feature/my-feature
```

Open a Pull Request against `main`.

## Coding Standards

### Error Handling
```rust
// Good
pub fn do_something() -> Result<(), VulturiusError> {
    // ...
    Err(VulturiusError::Io(std::io::Error::new(...)))
}

// Bad
pub fn do_something() {
    // ...
    panic!("error!");
}
```

### Logging
```rust
use tracing::{info, warn, error, debug};

info!("Scan complete: {} files", count);
warn!("Path blocked: {:?}", path);
error!("Failed to delete: {}", e);
debug!("Processing entry: {:?}", entry);
```

### Async Code
```rust
// Use tokio for async operations
use tokio::fs;
use tokio::sync::{mpsc, Mutex};

// Avoid blocking the runtime
tokio::task::spawn_blocking(|| {
    // CPU-intensive work
});
```

### Security
- Always validate paths through `PathGuard`
- Check permissions through `PermissionModel`
- Log security events through `AuditLogger`
- Never trust user-supplied paths

## Adding a New Feature

### 1. Define the Scope
- What problem does it solve?
- Which crate does it belong in? (usually `vulturius-core`)
- What's the API surface?

### 2. Implement in Core
```rust
// crates/vulturius-core/src/my_feature/mod.rs
pub mod my_module;
pub use my_module::*;
```

### 3. Wire Up
- Add module to `lib.rs`
- Add config options if needed
- Add CLI command if applicable
- Add Tauri command if GUI-relevant

### 4. Test
- Unit tests for all public functions
- Integration tests for cross-module interactions
- Security tests if file operations involved

### 5. Document
- Update `docs/API.md` with new types/functions
- Update `docs/USER_GUIDE.md` with usage examples
- Update `docs/ARCHITECTURE.md` if architecture changes

## Release Process

### Versioning
Follow [Semantic Versioning](https://semver.org/):
- **Major**: Breaking API changes
- **Minor**: New features (backward compatible)
- **Patch**: Bug fixes

### Tagging
```bash
git tag -a v0.2.0 -m "Release v0.2.0"
git push origin v0.2.0
```

The release workflow will:
1. Build release artifacts for all platforms
2. Create GitHub Release with changelog
3. Upload binaries
4. Publish to crates.io (if configured)

## Code of Conduct

- Be respectful and constructive
- Assume good intent
- Focus on the code, not the person
- If unsure, ask

## Getting Help

- **Issues**: [GitHub Issues](https://github.com/vulturius/vulturius/issues)
- **Discussions**: [GitHub Discussions](https://github.com/vulturius/vulturius/discussions)

## License

MIT License. By contributing, you agree to license your contributions under the same license.
