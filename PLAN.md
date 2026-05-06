# Vulturius — Design, Build & Test Plan

> *"A vulture cleans what is dead, leaving only what lives."*

A cross-platform disk analysis and intelligent cleanup tool with a blazing-fast Rust CLI core and a polished native GUI wrapper.

**Plan version: 3.2 | Last updated: 2026-05-05 (Session 3 — Phases 7-9 complete)**

---

## Table of Contents

1. [Product Vision](#product-vision)
2. [Technology Stack](#technology-stack)
3. [Architecture](#architecture)
4. [Core Modules](#core-modules)
5. [CLI Design](#cli-design)
6. [GUI Design](#gui-design)
7. [LLM Integration](#llm-integration)
8. [Phased Implementation](#phased-implementation)
9. [Testing Strategy](#testing-strategy)
10. [Project Structure](#project-structure)

---

## 1. Product Vision

### Problem
Users accumulate digital clutter: orphaned files, unused apps, stale caches, registry bloat, temp files from uninstalled software. Existing tools are either too aggressive (risk data loss), too dumb (no intelligence), or too platform-locked.

### Solution
**Vulturius** — a cross-platform disk sanitizer that:
- **Analyzes** disks with deep filesystem traversal and metadata inspection
- **Classifies** files by safety-to-delete confidence score
- **Identifies** unused applications and leftover artifacts
- **Cleans** intelligently with user-controlled risk tolerance
- **Augments** decisions with optional LLM reasoning for edge cases

### Key Principles
- **Safety first** — nothing is deleted without explicit or implicit user consent
- **Transparency** — every action is logged, reversible, and explainable
- **Performance** — native compilation, parallel scanning, minimal memory footprint
- **Intelligence** — heuristic + ML/LLM hybrid decision engine

---

## 2. Technology Stack

### Core Language: **Rust**

| Criterion | Why Rust |
|-----------|----------|
| Native performance | Zero-cost abstractions, no GC pauses |
| Cross-platform | Single codebase → Windows / macOS / Linux binaries |
| Memory safety | No segfaults during deep disk scans |
| Concurrency | Fearless parallel scanning with `tokio` |
| Ecosystem | Rich crate ecosystem for filesystem, CLI, GUI |
| Binary size | ~5-15MB standalone executables |
| No runtime dep | Ships as a single binary — no .NET, no JVM, no Python |

### CLI Framework
- **clap** — derive-based argument parsing with auto-generated help
- **clap_complete** — shell completion generation
- **indicatif** — progress bars and spinners

### GUI Framework: **Tauri 2 + Svelte 5** ✅

| Criterion | Why Tauri + Svelte |
|-----------|-------------------|
| Ultra Clean FUI | Web-based UI = pixel-perfect design freedom for sci-fi aesthetics |
| Small footprint | ~3-5MB total (vs 100MB+ for Electron) |
| Native backend | Rust backend = same core as CLI |
| Cross-platform | Single codebase, native windowing |
| Modern feel | CSS/HTML/WebGL = best-in-class UI polish + 3D effects |
| Svelte 5 runes | Reactive, minimal boilerplate |

### Key Rust Crates

| Area | Crate | Purpose |
|------|-------|---------|
| Filesystem traversal | `walkdir` + `ignore` | Recursive scanning with gitignore-style filters |
| File metadata | `sysinfo` + `filetime` | Access times, sizes, modification dates |
| Platform-specific | `windows` + `core-foundation` | Registry, LaunchServices, etc. |
| Async runtime | `tokio` | Parallel I/O for scanning |
| Serialization | `serde` + `serde_json` | Config, reports, IPC |
| Logging | `tracing` + `tracing-subscriber` | Structured logs |
| Config | `config` | TOML-based configuration |
| Database | `sqlite` (via `sqlx`) | Scan results, history, user preferences |
| LLM | `reqwest` | OpenAI/Anthropic/Ollama API calls |
| LLM encryption | `aes-gcm` + `rand` | AES-256-GCM API key encryption |
| LLM streaming | `futures` | Real-time chunk delivery |
| GUI | Tauri 2 | Desktop app shell with Rust backend |
| Hashing | `sha2` | File deduplication detection |

### FUI-Specific Frontend Tech

| Area | Technology | Purpose |
|------|-----------|---------|
| Animations | CSS keyframes + Svelte transitions | Neon glow, scan-lines, pulse effects |
| Glassmorphism | CSS `backdrop-filter` | Frosted glass panels |
| Neon glow | CSS `box-shadow` + SVG filters | Glowing borders, text, indicators |
| Scan-line overlay | CSS `repeating-linear-gradient` | CRT/hologram aesthetic |
| Data visualization | CSS grid + Canvas API | Heatmaps, radar sweeps, orbit rings |
| Sound FX | Web Audio API (planned) | Optional sci-fi UI feedback tones |

---

## 3. Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Vulturius GUI (Tauri 2)               │
│  ┌───────────┐  ┌───────────┐  ┌───────────┐           │
│  │ Dashboard │  │ Scanner   │  │ Analyzer  │           │
│  │ (Orb+Map) │  │ (Radar)   │  │ (Heatmap) │           │
│  └───────────┘  └───────────┘  └───────────┘           │
│  ┌───────────┐  ┌───────────┐  ┌───────────┐           │
│  │ Cleaner   │  │ Apps      │  │ Settings  │           │
│  │ (Particles)│ │ (Orbits)  │  │ (Config)  │           │
│  └───────────┘  └───────────┘  └───────────┘           │
│                          │                              │
│                  ┌───────┴───────┐                      │
│                  │  Tauri Bridge │ 16 commands           │
│                  └───────┬───────┘                      │
└──────────────────────────┼──────────────────────────────┘
                           │ IPC (JSON-RPC)
┌──────────────────────────┼──────────────────────────────┐
│                  Vulturius Core (Rust Library)            │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────────┐ │
│  │ Scanner  │ │Analyzer  │ │ Cleaner  │ │ LLM Engine │ │
│  │ Engine   │ │ Engine   │ │ Engine   │ │ (production)│ │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ └─────┬──────┘ │
│       │            │            │              │        │
│  ┌────┴────────────┴────────────┴──────────────┴────┐  │
│  │              Platform Abstraction Layer           │  │
│  │  ┌────────┐ ┌────────┐ ┌────────┐ ┌──────────┐  │  │
│  │  │Windows │ │ macOS  │ │ Linux  │ │ Common   │  │  │
│  │  │Adapter │ │Adapter │ │Adapter │ │ Utils    │  │  │
│  │  └────────┘ └────────┘ └────────┘ └──────────┘  │  │
│  └──────────────────────────────────────────────────┘  │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐               │
│  │ SQLite   │ │ Config   │ │ Logging  │               │
│  │ (SQLx)   │ │ (TOML)   │ │ (tracing)│               │
│  └──────────┘ └──────────┘ └──────────┘               │
└────────────────────────────────────────────────────────┘
         │                                    │
    ┌────┴────┐                        ┌──────┴──────┐
    │  CLI    │                        │  Scheduled  │
    │ (binary)│                        │  Daemon     │
    └─────────┘                        │ (Phase 9)   │
                                       └─────────────┘
```

---

## 4. Core Modules

### 4.1 Scanner Engine ✅

**Purpose:** Traverse filesystems and collect file metadata.

**Capabilities:**
- Recursive directory traversal with configurable depth
- Parallel scanning using `tokio` worker pool
- File classification by type (document, image, log, temp, cache, etc.)
- Metadata collection: size, last accessed, last modified, created, permissions
- Mount point / drive detection across platforms
- Exclusion patterns (system dirs, user-defined whitelists)
- Real-time progress reporting via channels

### 4.2 Analyzer Engine ✅

**Purpose:** Evaluate each file/app for safety-to-delete scoring.

**Scoring System (0.0 — 1.0):**

```
Confidence Score Factors:
├── Age Factor (0.30 weight)       — How long since last access?
├── Location Factor (0.25 weight)  — Is it in a temp/cache dir?
├── Type Factor (0.20 weight)      — Known disposable type?
├── Dependency Factor (0.15 weight) — Anything referencing this?
├── User Pattern Factor (0.10 weight) — Historical behavior
└── LLM Factor (0.00 — 0.10 bonus) — Optional LLM evaluation
```

**Risk Tolerance Levels:**

| Level | Description | Min Score to Recommend |
|-------|-------------|----------------------|
| `paranoid` | Only obvious junk | 0.90+ |
| `conservative` | Safe with margin | 0.75+ |
| `balanced` | Default recommendation | 0.60+ |
| `aggressive` | Free up maximum space | 0.40+ |
| `nuclear` | Everything flagged | 0.10+ |

### 4.3 Cleaner Engine ✅

**Purpose:** Execute cleanup operations safely.

**Safety Mechanisms:**
- **Quarantine mode:** Move to `.vulturius/quarantine/` instead of deleting
- **Dry-run mode:** Report what would be done without acting
- **Undo journal:** Every action logged to SQLite for reversal
- **System file protection:** Hard-blocked paths never touched
- **Confirmation thresholds:** Large deletions require explicit approval

### 4.4 LLM Engine ✅ (Production-Ready)

**Purpose:** Augment analysis for ambiguous cases.

**Production Features (Phase 7):**
- **OpenAI provider** — Full API integration with retry, streaming, rate limiting
- **Anthropic (Claude) provider** — Claude 3 Opus/Sonnet/Haiku support
- **Ollama provider** — Local model auto-discovery, batch analysis
- **Provider fallback chain** — Tries multiple providers in order
- **Rate limiting** — Token bucket algorithm per provider
- **Retry with exponential backoff** — Configurable retry policy with jitter
- **Streaming responses** — Real-time chunk delivery via tokio channels
- **Decision caching** — LRU cache keyed on file metadata
- **AES-256-GCM key encryption** — Encrypted API key storage at rest
- **Cost estimation** — Per-model cost tracking and budget enforcement
- **Natural language queries** — NL → scan config conversion
- **84 unit tests** — Full coverage across all LLM modules

### 4.5 Windows Registry ✅

**Purpose:** Detect and clean orphaned registry entries.

**Capabilities:**
- Scan HKLM/HKCU Uninstall, Run, Services keys
- Cross-reference orphans against installed apps
- Backup/restore via .reg files
- Dry-run support

---

## 5. CLI Design ✅

### Command Structure

```
vulturius <COMMAND> [SUBCOMMAND] [OPTIONS]

Commands:
  scan        Scan disk/filesystem for analyzable data
  analyze     Analyze scan results and generate recommendations
  clean       Execute cleanup operations
  app         Application analysis and management
  registry    Windows registry analysis and cleanup
  quarantine  Manage quarantined files
  report      Generate and view reports
  config      Manage configuration
  history     View cleanup history
  undo        Reverse previous cleanup operations
```

---

## 6. GUI Design ✅

### Design Philosophy: Ultra Clean FUI (Futuristic User Interface)

**FUI = Futuristic User Interface** — the visual language of sci-fi cinema.

Core aesthetic principles:
- **Holographic depth** — glassmorphism, floating panels, backdrop blur
- **Neon glow accents** — cyan/teal primary, amber warnings, crimson danger
- **Data viz as primary UI** — heatmaps, particle flows, radial gauges
- **Minimal typography** — JetBrains Mono, oversized numbers, sparse labels
- **Ambient animation** — breathing glows, scan-lines, pulsing indicators
- **Radial & circular layouts** — dials, concentric rings, orbiting data
- **Transparency everywhere** — frosted glass, layered opacity
- **Dark void canvas** — deep black (#000008) background

### Implemented Pages (6/7)

| Page | Status | FUI Elements |
|------|--------|-------------|
| Dashboard | ✅ | Central storage orb, quick stats, storage map, activity stream |
| Scanner | ✅ | Radar sweep animation, scan controls, live progress |
| Analyzer | ✅ | Confidence heatmap, risk dial, analysis controls |
| Cleaner | ✅ | Particle destruction FX, dry-run/quarantine toggles, results |
| Apps | ✅ | Orbit rings, usage indicators, filter system |
| Settings | ✅ | General, Scan, Analysis, LLM, Cleanup sections |
| Registry | ❌ Planned | Neon tree, orphan visualization (Windows-only) |

### Implemented Components (10)

| Component | Purpose |
|-----------|---------|
| NavRing | Circular navigation with active indicators |
| GlassPanel | Reusable glass-morphism container |
| NeonButton | Color-coded neon buttons with glow effects |
| StorageOrb | 3D-style storage visualization with orbiting data points |
| RadarSweep | Animated radar sweep for scan visualization |
| Heatmap | Confidence score grid visualization |
| RiskDial | Interactive risk tolerance selector |
| ParticleFX | Cleanup destruction animation |
| OrbitRing | App usage orbit visualization |
| ActivityStream | Recent activity feed |

### Color System (FUI Neon Palette)

```
Void Black:     #000008  — canvas background
Panel Glass:    #0A0A1AFF — floating panels (with backdrop-blur)

Neon Cyan:      #00F0FF  — primary glow, active elements
Neon Teal:      #00D4AA  — secondary actions, progress
Neon Amber:     #FFB800  — warnings, caution items
Neon Crimson:   #FF2D55  — danger, risky items
Neon Purple:    #B829FF  — LLM/AI features
Neon White:     #E8E8FF  — text, labels, data readouts
```

---

## 7. LLM Integration

### Architecture (Phase 7 — Production-Ready) ✅

```
┌─────────────────────────────────────────────────┐
│              LlmEngine (Coordinator)              │
│  ┌─────────────┐  ┌──────────────┐              │
│  │Decision Cache│  │Budget/Cost   │              │
│  │(LRU, 10K)   │  │Tracking      │              │
│  └─────────────┘  └──────────────┘              │
│                    │                             │
│  ┌─────────────────┴──────────────────────────┐ │
│  │         FallbackChain (Ordered)             │ │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐   │ │
│  │  │ OpenAI   │ │ Anthropic│ │  Ollama  │   │ │
│  │  │ Provider │ │ Provider │ │ Provider │   │ │
│  │  └────┬─────┘ └────┬─────┘ └────┬─────┘   │ │
│  │       │            │            │           │ │
│  │  ┌────┴────────────┴────────────┴────────┐ │ │
│  │  │  RateLimiter + Retry (backoff+jitter) │ │ │
│  │  └───────────────────────────────────────┘ │ │
│  └────────────────────────────────────────────┘ │
│                    │                            │
│  ┌─────────────────┴──────────────────────────┐ │
│  │         KeyStore (AES-256-GCM)              │ │
│  │  Encrypted API key persistence              │ │
│  └────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────┘
```

### Provider Details

| Provider | Features | Rate Limits |
|----------|----------|-------------|
| **OpenAI** | GPT-4o, GPT-4o-mini, streaming SSE, retry | 60 RPM, 1M TPM, 10 concurrent |
| **Anthropic** | Claude 3 Opus/Sonnet/Haiku, streaming | 50 RPM, 500K TPM, 8 concurrent |
| **Ollama** | Local models, auto-discovery, batch | Unlimited (local) |

### Rate Limiting Algorithm
- **Token bucket** per provider for requests and tokens
- **Concurrent request** limiting per provider
- **Automatic backoff** when limits reached (100ms retry)

### Retry Policy
- **Max retries:** 3 (configurable)
- **Base delay:** 1 second
- **Max delay:** 30 seconds
- **Jitter:** Random ±500ms to prevent thundering herd

### Streaming
- **SSE parsing** for OpenAI/Anthropic streaming
- **Chunk types:** Text, Progress, Analysis, Error, Done
- **Real-time progress** tracking with percentage
- **Callback-based** processing for UI integration

### Cost Estimation

| Model | Estimated Cost/Call |
|-------|-------------------|
| GPT-4o-mini | $0.0001 |
| GPT-4o | $0.001 |
| GPT-4 | $0.003 |
| GPT-3.5 | $0.00005 |
| Claude 3 Opus | $0.015 |
| Claude 3 Sonnet | $0.003 |
| Claude 3 Haiku | $0.00025 |
| Local (Ollama) | Free |

---

## 8. Phased Implementation

### Completed Phases ✅

| Phase | Description | Status | Tests |
|-------|-------------|--------|-------|
| Phase 1 | Foundation (workspace, platform, scanner, config, CLI, DB) | ✅ Complete | — |
| Phase 2 | Core Engine (parallel scanner, classifier, analyzer, scoring) | ✅ Complete | — |
| Phase 3 | Cleanup Engine (cleaner, quarantine, undo, CLI commands) | ✅ Complete | — |
| Phase 4 | Windows Registry (orphan detection, backup/restore, cleaner) | ✅ Complete | 11 |
| Phase 5 | LLM Integration (OpenAI, Ollama, caching, NL queries) | ✅ Complete | 8 |
| Phase 6 | Tauri GUI (Svelte 5, FUI, 16 commands, 6 pages, 10 components) | ✅ Complete | — |
| Phase 7 | LLM Providers (production-grade, rate limiting, streaming, encryption) | ✅ Complete | 50 |
| Phase 8 | Reporting Engine (HTML/JSON with charts, trend analysis) | ✅ Complete | 6 |
| Phase 9 | Scheduling (cron, auto-cleanup, notifications, email, startup) | ✅ Complete | 11 |

### Remaining Phases

### Phase 9: Scheduling ✅
- [x] Cron-like scheduler with interval parsing
- [x] Auto-cleanup with configurable rules
- [x] Desktop notifications (platform-native)
- [x] Email report delivery (SMTP framework, disk fallback)
- [x] Startup integration (Windows registry, macOS LaunchAgent, Linux systemd)
- [x] 5 new Rust files in scheduler/
- [x] 11 new unit tests (117 total, all passing)

### Phase 10: Performance Hardening
- [ ] Benchmark suite (scan speed, memory usage)
- [ ] Memory profiling and leak detection
- [ ] Scan optimization (skip unchanged dirs)
- [ ] Parallel I/O tuning
- [ ] Database query optimization

### Phase 11: Security Audit
- [ ] Sandboxed file operations
- [ ] Permission model (admin vs user)
- [ ] Path traversal protection
- [ ] API key encryption ✅ (done in Phase 7)
- [ ] Audit logging

### Phase 12: Packaging & Release
- [ ] CI/CD workflows (GitHub Actions: Ubuntu, macOS, Windows)
- [ ] Build scripts (cross-compilation)
- [ ] Installer packages (NSIS, DMG, AppImage)
- [ ] Documentation (architecture, API, user guide, contributing)
- [ ] Internationalization (i18n)
- [ ] E2E tests
- [ ] Release pipeline (GitHub Releases, auto-versioning)

---

## 9. Testing Strategy

### Current Test Coverage ✅
- **117 unit tests** passing across vulturius-core (up from 90)
- Scanner engine (15 tests)
- Analyzer + scoring (12 tests)
- Cleaner + quarantine (8 tests)
- Registry module (11 tests)
- LLM integration (8 tests)
- **LLM Phase 7** (50 new tests):
  - Provider prompts + parsing (12 tests)
  - Rate limiter (6 tests)
  - Retry with backoff (3 tests)
  - Fallback chain (4 tests)
  - Key store encryption (6 tests)
  - Streaming (4 tests)
  - Engine integration (11 tests)
- Full workspace compiles cleanly

### Test Pyramid

```
        ┌─────────────┐
        │   E2E Tests  │     ~5%  — Full integration scenarios (Phase 12)
        ├─────────────┤
        │ Integration │     ~20% — Multi-module interactions (Phase 10)
        ├─────────────┤
        │  Unit Tests  │     ~75% — Individual functions/types (✅ 84 tests)
        └─────────────┘
```

### Performance Benchmarks (Phase 10)
```
- Scan 100K files: target < 30s on modern hardware
- Analyze 100K entries: target < 10s
- Memory usage: < 200MB for 1M file scan
- Concurrency scaling: linear up to 8 threads
```

---

## 10. Project Structure

```
vulture/
├── Cargo.toml                    # Workspace root (3 crates)
├── PROGRESS.md                   # Development progress tracker
├── PLAN.md                       # This document
│
├── crates/
│   ├── vulturius-core/           # Core library ✅
│   │   ├── scanner/              # Parallel filesystem scanner ✅
│   │   ├── analyzer/             # Confidence scoring + app detection ✅
│   │   ├── cleaner/              # Cleanup + quarantine + undo ✅
│   │   ├── llm/                  # Production LLM engine ✅ (Phase 7)
│   │   │   ├── mod.rs            # Engine + DecisionCache + budget tracking
│   │   │   ├── provider/
│   │   │   │   ├── mod.rs        # Types, prompts, JSON parsing
│   │   │   │   ├── openai.rs     # GPT-4o/mini, streaming SSE, retry
│   │   │   │   ├── anthropic.rs  # Claude 3 Opus/Sonnet/Haiku
│   │   │   │   └── ollama.rs     # Local models, auto-discovery
│   │   │   ├── fallback.rs       # Provider fallback chain
│   │   │   ├── key_store.rs      # AES-256-GCM encrypted key storage
│   │   │   ├── rate_limiter.rs   # Token bucket rate limiting
│   │   │   ├── retry.rs          # Exponential backoff with jitter
│   │   │   └── streaming.rs      # Real-time chunk delivery
│   │   ├── reporter/             # Reporting engine ✅ (Phase 8)
│   │   │   ├── mod.rs            # Reporter coordinator + config
│   │   │   ├── text_report.rs    # Text reports with category breakdown
│   │   │   ├── html_report.rs    # HTML reports with inline SVG charts
│   │   │   ├── json_report.rs    # JSON export for automation
│   │   │   └── trend.rs          # Cleanup trend analysis
│   │   ├── platform/             # Cross-platform abstraction ✅
│   │   ├── registry/             # Windows registry analysis ✅
│   │   ├── config.rs             # TOML configuration ✅
│   │   ├── scheduler/            # Scheduler ✅ (Phase 9)
│   │   │   ├── mod.rs            # Scheduler coordinator + config
│   │   │   ├── cron.rs           # Cron-like schedule parser
│   │   │   ├── notification.rs   # Desktop notifications (platform-native)
│   │   │   ├── report_email.rs   # Email report delivery
│   │   │   ├── rule.rs           # Auto-cleanup rules
│   │   │   └── startup.rs        # Boot integration
│   │   ├── types.rs              # Core data types ✅
│   │   ├── error.rs              # Error handling ✅
│   │   └── db.rs                 # SQLite (SQLx) ✅
│   │
│   ├── vulturius-cli/            # CLI binary ✅
│   │   └── src/                  # 10 subcommands ✅
│   │
│   └── vulturius-gui/            # Tauri 2 desktop app ✅
│       ├── src/
│       │   ├── main.rs           # Tauri setup + state management ✅
│       │   └── commands/         # 16 Tauri commands ✅
│       │       ├── scan.rs       # start_scan, get_scan_status, cancel_scan ✅
│       │       ├── analyze.rs    # analyze_files, get_analysis_results ✅
│       │       ├── clean.rs      # execute_cleanup, get_cleanup_status, get_history ✅
│       │       └── system.rs     # get_drives, get_system_info, get_config, get_apps, quarantine ✅
│       ├── src-tauri/            # Tauri config ✅
│       └── frontend/             # Svelte 5 + Tailwind ✅
│           ├── src/lib/components/  # 10 FUI components ✅
│           ├── src/lib/pages/       # 6 pages ✅
│           ├── src/lib/stores/      # Global state ✅
│           └── src/styles/          # FUI design system ✅
│
├── tests/                        # (Phase 10-12)
├── scripts/                      # (Phase 12)
├── docs/                         # (Phase 12)
└── .github/                      # (Phase 12)
```

---

## Appendix A: Intelligent Defaults

| Parameter | Default | Rationale |
|-----------|---------|-----------|
| Risk tolerance | `balanced` | Safe for most users |
| Unused app threshold | 90 days | 3 months is reasonable |
| Min file age | 30 days | Avoids deleting recent work |
| Max threads | Auto (CPU count) | Utilizes hardware |
| Quarantine | Enabled | Safety net |
| LLM | Disabled | Opt-in, cost control |
| Scan hidden files | Off | Reduce noise |
| Scan system dirs | Off | Safety |
| Hash collection | Off | Expensive, opt-in for dedup |

## Appendix B: Blocked Paths (Never Touched)

```
Windows:
  C:\Windows\System32\*
  C:\Windows\SysWOW64\*
  C:\Pagefile.sys
  C:\hiberfil.sys
  C:\Recovery\*
  NTUSER.DAT

macOS:
  /System/*
  /usr/lib/*
  /usr/share/*
  /Library/Extensions/*

Linux:
  /bin/*
  /sbin/*
  /lib/*
  /usr/lib/*
  /boot/*
  /proc/*
  /sys/*
```

## Appendix C: File Categories

| Category | Examples | Default Safety |
|----------|----------|----------------|
| Temp | `*.tmp`, `Temp/`, `/tmp/` | High |
| Cache | `Cache/`, `.cache/`, `*.cache` | High |
| Log | `*.log`, `Logs/` | Medium-High |
| Crash Dump | `*.dmp`, `*.crash`, `*.trace` | High |
| Thumbnail | `*.thumb`, `Thumbs.db`, `.DS_Store` | High |
| Download Helper | `*.part`, `*.crdownload`, `*.incomplete` | High |
| Package Cache | `apt/`, `pip/`, `npm/`, `cargo/` | Medium |
| Browser Data | Cookies, history, extensions cache | Medium |
| Old Backups | `*.bak`, `*.old`, `*.backup` | Medium |
| Duplicate | Same hash, older copy | Medium-High |
| Orphaned | No parent app installed | Medium |
| Unknown | Unrecognized type | Low (LLM recommended) |

## Appendix D: LLM Provider Configuration

```toml
[llm]
enabled = true
provider = "openai"          # "openai" | "anthropic" | "ollama"
model = "gpt-4o-mini"       # Model variant
api_key = "sk-..."          # Or use encrypted key store
max_calls_per_scan = 50     # Budget cap
timeout_seconds = 30        # Per-call timeout
cache_decisions = true      # LRU cache enabled

# Fallback chain (multiple providers)
[llm.providers]
primary = { provider = "openai", model = "gpt-4o-mini" }
fallback = [
    { provider = "anthropic", model = "claude-3-haiku-20240307" },
    { provider = "ollama", model = "llama3" },
]

# Retry policy
[llm.retry]
max_retries = 3
base_delay_seconds = 1
max_delay_seconds = 30
jitter_ms = 500

# Rate limits (per provider)
[llm.rate_limits]
openai = { rpm = 60, tpm = 1000000, max_concurrent = 10 }
anthropic = { rpm = 50, tpm = 500000, max_concurrent = 8 }
```

---

*Plan version: 3.2 | Last updated: 2026-05-05 (Session 3) | Phases 1-9 complete, 117 tests passing*
