# Vulturius — Architecture Documentation

## Overview

Vulturius is a cross-platform disk cleanup and system optimization tool. It scans filesystems, analyzes files using a multi-factor confidence scoring system, optionally leverages LLMs for intelligent classification, and provides safe cleanup operations with quarantine and undo support.

## Project Structure

```
Vulture/
├── Cargo.toml                      # Workspace manifest
├── crates/
│   ├── vulturius-core/             # Core library (all business logic)
│   │   ├── src/
│   │   │   ├── lib.rs              # Module exports
│   │   │   ├── analyzer/           # File analysis & confidence scoring
│   │   │   ├── cleaner/            # Cleanup engine (delete, quarantine, undo)
│   │   │   ├── config/             # Application configuration (TOML)
│   │   │   ├── db/                 # SQLite database layer
│   │   │   ├── error.rs            # Error types (thiserror)
│   │   │   ├── llm/                # LLM integration (providers, caching, keys)
│   │   │   ├── perf/               # Performance optimization (async I/O, benchmarks)
│   │   │   ├── platform/           # Cross-platform abstraction layer
│   │   │   ├── registry/           # Windows Registry analysis & cleanup
│   │   │   ├── reporter/           # Report generation (HTML, JSON, text)
│   │   │   ├── scanner/            # Filesystem scanner & classifier
│   │   │   ├── scheduler/          # Auto-scan & auto-cleanup scheduling
│   │   │   ├── security/           # Security audit (sandbox, permissions, audit log)
│   │   │   └── types.rs            # Core data types
│   │   └── benches/                # Criterion benchmark harness
│   ├── vulturius-cli/              # CLI application
│   └── vulturius-gui/              # Tauri 2 desktop GUI
├── .github/workflows/              # CI/CD pipelines
└── docs/                           # Documentation
```

## Core Architecture

### 1. Scanner Layer

```
User → ScanTarget → WalkBuilder → FileEntry → ScanResult
                              ↓
                        FileClassifier
                              ↓
                        FileCategory (temp, cache, log, etc.)
```

- **Parallel scanning** via `tokio::spawn` — one worker per target
- **Shared state** protected by `Arc<Mutex<SharedState>>`
- **Progress reporting** via `mpsc::Sender<ScanProgress>`
- **Exclusion filtering** by path patterns and system path protection

### 2. Analyzer Layer

```
FileEntry → 5-Factor Scoring → Confidence Score → Recommendation
         │                    │
         ├─ File age          ├─ Weighted sum
         ├─ File size         ├─ Risk tolerance adjustment
         ├─ File category     └─ LLM override (optional)
         ├─ Access frequency
         └─ User context
```

- **5-factor weighted scoring**: age (25%), size (20%), category (25%), access frequency (15%), context (15%)
- **Risk tolerance levels**: Paranoid, Conservative, Balanced, Aggressive, Nuclear
- **LLM integration**: Optional override with decision caching

### 3. Cleanup Layer

```
CleanupPlan → Cleaner → Action → Result
              │         │
              │         ├─ Delete (permanent)
              │         ├─ Quarantine (safe)
              │         └─ Truncate (logs)
              │
              └─ UndoJournal (reversibility)
```

- **Three modes**: delete, quarantine, dry-run
- **Undo journal**: Records all operations for reversal
- **Quarantine**: Moves files to isolated directory with metadata

### 4. Database Layer

```
SqlitePool → PRAGMA Tuning → Tables → Queries
            (WAL mode)      (7 tables)
```

- **SQLite with WAL mode** for concurrent read/write
- **7 tables**: scans, file_entries, file_analysis, cleanup_history, cleanup_journal, app_registry, quarantine
- **11 performance indexes** for common query patterns
- **PRAGMA tuning**: cache=-8192, sync=NORMAL, mmap=256MB

### 5. LLM Layer

```
LlmRequest → Provider Chain → Response
            │                │
            ├─ OpenAI        ├─ Streaming (SSE)
            ├─ Anthropic     ├─ Rate limiting
            ├─ Ollama        └─ Retry with backoff
            │
            └─ Decision Cache (hash-based)
```

- **Provider fallback chain**: Automatic failover between providers
- **Rate limiting**: Token bucket per provider (RPM, TPM, concurrent)
- **Retry policy**: Exponential backoff with jitter
- **Streaming responses**: Tokio mpsc channels
- **AES-256-GCM key encryption**: API keys encrypted at rest

### 6. Performance Layer

```
AsyncHasher → Semaphore → spawn_blocking → SHA-256
            (concurrency limit)
```

- **Async I/O tuning**: Buffered reads (128KB default), configurable buffer sizes
- **Worker pool auto-tuning**: Scales by CPU cores + available RAM
- **Storage type detection**: SSD/HDD/Network with per-type concurrency
- **Incremental scanning**: mtime-based directory change detection
- **Criterion benchmarks**: 8 benchmark groups, 20+ individual benchmarks

### 7. Security Layer

```
PathRequest → PathGuard → PermissionModel → AuditLogger
            │            │                │
            │            ├─ Traversal     ├─ Privilege
            │            ├─ Null bytes    ├─ Operation
            │            └─ Depth limit   └─ Logging
```

- **Path sandboxing**: Allowed root enforcement with blocklist
- **Permission model**: 3-tier (Administrator/User/ReadOnly)
- **Path traversal protection**: `..` detection, null bytes, depth limits, Windows device blocking
- **Audit logging**: Bounded in-memory (10K entries) with optional disk persistence

### 8. Scheduling Layer

```
CronScheduler → Scheduler → Auto-Scan / Auto-Cleanup
              │            │
              ├─ Interval  ├─ Desktop notifications
              ├─ Daily     ├─ Email reports
              └─ Weekly    └─ Startup integration
```

- **Cron-like scheduling**: Interval parsing ("7d", "24h", "6h", "30m")
- **Auto-cleanup rules**: 6 default rule sets (temp, cache, log, crash dump, thumbnail, old backup)
- **Desktop notifications**: Platform-native (Windows toast, macOS osascript, Linux notify-send)
- **Email reports**: SMTP framework with disk fallback

## Cross-Platform Abstraction

```rust
trait PlatformOps {
    fn is_system_path(&self, path: &Path) -> bool;
    fn expand_env_vars(&self, s: &str) -> String;
    fn get_user_home(&self) -> PathBuf;
    fn get_temp_dirs(&self) -> Vec<PathBuf>;
    // ...
}
```

Implemented for Windows, macOS, and Linux with platform-specific behaviors.

## Configuration

TOML-based configuration with sections:
- `general`: Language, output format, log level, max threads
- `scan`: Depth, exclusions, hidden/system scan, hash collection
- `analysis`: Risk tolerance, thresholds, LLM settings
- `cleanup`: Quarantine, backup, confirmation threshold
- `llm`: Provider, model, API key, caching
- `schedule`: Auto-scan/cleanup, notifications, email, startup

## Data Flow

```
CLI/GUI → Scanner → FileEntries → Analyzer → Analysis → Cleaner → Result
                              ↓              ↓
                          Database        Reports
                              ↓
                          Scheduler (auto-cycles)
```

## Testing

- **171 unit tests** across all modules
- **Criterion benchmarks** for performance regression detection
- **Cross-platform CI** on Ubuntu, Windows, macOS
- **Security tests** for sandbox, permissions, path traversal

## Dependencies

| Category | Key Dependencies |
|----------|-----------------|
| Async | tokio, futures |
| Database | sqlx (SQLite) |
| Serialization | serde, serde_json |
| CLI | clap, indicatif |
| GUI | Tauri 2, Svelte 5 |
| LLM | reqwest |
| Security | aes-gcm, rand |
| Logging | tracing, tracing-subscriber |
| Filesystem | walkdir, ignore, filetime |
| Crypto | sha2, hex |
| Config | config (TOML) |
| Benchmarking | criterion, dhat |
