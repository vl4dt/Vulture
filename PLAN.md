# Vulturius — Design, Build & Test Plan

> *"A vulture cleans what is dead, leaving only what lives."*

A cross-platform disk analysis and intelligent cleanup tool with a blazing-fast Rust CLI core and a polished native GUI wrapper.

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

### GUI Framework: **Tauri 2 + Svelte 5**

| Criterion | Why Tauri + Svelte |
|-----------|-------------------|
| Ultra Clean FUI | Web-based UI = pixel-perfect design freedom for sci-fi aesthetics |
| Small footprint | ~3-5MB total (vs 100MB+ for Electron) |
| Native backend | Rust backend = same core as CLI |
| Cross-platform | Single codebase, native windowing |
| Modern feel | CSS/HTML/WebGL = best-in-class UI polish + 3D effects |
| Svelte 5 runes | Reactive, minimal boilerplate |

### Alternative GUI considered and rejected
- **Iced (Rust native)** — too limited for sci-fi custom UI and 3D effects
- **Flutter** — Dart adds a second language, larger binary
- **Qt/Rust (CXX)** — C interop complexity, heavy dependency, hard to achieve holographic look

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
| LLM | `reqwest` + `async-openai` | OpenAI-compatible API calls |
| IPC | Tauri commands | CLI ↔ GUI bridge |
| Archiving | `zip` / `tar` | Quarantine backup before delete |
| Hashing | `sha2` | File deduplication detection |

### FUI-Specific Frontend Tech

| Area | Technology | Purpose |
|------|-----------|---------|
| 3D orbs/gauges | **Three.js / Svelte 3D** | Central storage orb, 3D data viz |
| Particle FX | **Canvas API / PixiJS** | File destruction effects, scan particles |
| Animations | **Framer Motion (Svelte port) / Motion One** | Smooth sci-fi transitions |
| Glassmorphism | **CSS backdrop-filter** | Frosted glass panels |
| Neon glow | **CSS box-shadow + SVG filters** | Glowing borders, text, indicators |
| Sound FX | **Web Audio API** | Optional sci-fi UI feedback tones |
| Scan-line overlay | **CSS repeating-linear-gradient** | CRT/hologram aesthetic |

---

## 3. Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Vulturius GUI (Tauri)                 │
│  ┌───────────┐  ┌───────────┐  ┌───────────┐           │
│  │  Scanner   │  │  Reviewer │  │  Dashboard│           │
│  │  Panel     │  │  Panel    │  │  Panel    │           │
│  └───────────┘  └───────────┘  └───────────┘           │
│                          │                              │
│                  ┌───────┴───────┐                      │
│                  │  Tauri Bridge │                      │
│                  └───────┬───────┘                      │
└──────────────────────────┼──────────────────────────────┘
                           │ IPC (JSON-RPC)
┌──────────────────────────┼──────────────────────────────┐
│                  Vulturius Core (Rust Library)            │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────────┐ │
│  │ Scanner  │ │Analyzer  │ │ Cleaner  │ │ LLM Engine │ │
│  │ Engine   │ │ Engine   │ │ Engine   │ │ (optional) │ │
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
│  │ Store    │ │ Manager  │ │ System   │               │
│  └──────────┘ └──────────┘ └──────────┘               │
└────────────────────────────────────────────────────────┘
         │                                    │
    ┌────┴────┐                        ┌──────┴──────┐
    │  CLI    │                        │  Scheduled  │
    │ (binary)│                        │  Daemon     │
    └─────────┘                        └─────────────┘
```

### Layer Responsibilities

1. **Platform Abstraction Layer** — Normalizes OS-specific APIs into a unified interface
2. **Core Engines** — Platform-agnostic logic for scanning, analysis, cleaning
3. **Tauri Bridge** — Exposes core as JSON-RPC commands to the GUI
4. **CLI Binary** — Thin wrapper around core with terminal output
5. **GUI** — Svelte frontend consuming Tauri commands, FUI rendering layer

---

## 4. Core Modules

### 4.1 Scanner Engine

**Purpose:** Traverse filesystems and collect file metadata.

**Capabilities:**
- Recursive directory traversal with configurable depth
- Parallel scanning using `tokio` worker pool (configurable concurrency)
- File classification by type (document, image, log, temp, cache, etc.)
- Metadata collection: size, last accessed, last modified, created, permissions
- Mount point / drive detection across platforms
- Exclusion patterns (system dirs, user-defined whitelists)
- Real-time progress reporting via channels

**Key Data Structures:**

```rust
pub struct ScanTarget {
    pub path: PathBuf,
    pub scan_type: ScanType,        // Full, Quick, Custom
    pub exclusions: Vec<Pattern>,
    pub max_depth: Option<usize>,
    pub file_types: Option<Vec<FileCategory>>,
}

pub struct FileEntry {
    pub path: PathBuf,
    pub size: u64,
    pub file_type: FileType,
    pub category: FileCategory,
    pub last_accessed: SystemTime,
    pub last_modified: SystemTime,
    pub created: SystemTime,
    pub permissions: FileMode,
    pub is_system: bool,
    pub is_hidden: bool,
    pub mime_type: Option<String>,
    pub hash: Option<String>,       // for dedup detection
}

pub struct ScanResult {
    pub entries: Vec<FileEntry>,
    pub total_size: u64,
    pub total_files: usize,
    pub scan_duration: Duration,
    pub errors: Vec<ScanError>,
}
```

### 4.2 Analyzer Engine

**Purpose:** Evaluate each file/app for safety-to-delete scoring.

**Scoring System (0.0 — 1.0):**

```
Confidence Score Factors:
├── Age Factor (0.30 weight)
│   └── How long since last access? Older = higher score
├── Location Factor (0.25 weight)
│   └── Is it in a temp/cache dir? Higher score
├── Type Factor (0.20 weight)
│   └── Is it a known disposable type (log, cache, temp)? Higher score
├── Dependency Factor (0.15 weight)
│   └── Is anything referencing this file? Lower score if yes
├── User Pattern Factor (0.10 weight)
│   └── User's historical behavior with similar files
└── LLM Factor (0.00 — 0.10 bonus)
    └── Optional: LLM evaluation for ambiguous files
```

**Risk Tolerance Levels:**

| Level | Description | Min Score to Recommend |
|-------|-------------|----------------------|
| `paranoid` | Only obvious junk | 0.90+ |
| `conservative` | Safe with margin | 0.75+ |
| `balanced` | Default recommendation | 0.60+ |
| `aggressive` | Free up maximum space | 0.40+ |
| `nuclear` | Everything flagged | 0.10+ |

**App Usage Analysis:**

```rust
pub struct AppAnalysis {
    pub name: String,
    pub install_path: PathBuf,
    pub last_launched: Option<SystemTime>,
    pub launch_count: usize,
    pub total_size: u64,
    pub usage_score: f64,       // 0.0 (never used) to 1.0 (daily)
    pub uninstall_safe: bool,
    pub leftover_files: Vec<PathBuf>,
    pub registry_entries: Vec<String>, // Windows-specific
    pub recommendation: AppRecommendation,
}
```

**Platform-specific App Detection:**
- **Windows:** Registry (`HKLM/HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall`), Start Menu, Program Files
- **macOS:** `/Applications`, `~/Applications`, LaunchAgents/Daemons, `.app` bundles
- **Linux:** `/usr/bin`, `~/.local/bin`, `snap`, `flatpak`, `dpkg`/`pacman`/`rpm` databases

### 4.3 Cleaner Engine

**Purpose:** Execute cleanup operations safely.

**Operations:**

| Operation | Description | Platforms |
|-----------|-------------|-----------|
| `file_delete` | Remove files with optional quarantine | All |
| `app_uninstall` | Full app removal + leftover cleanup | All |
| `temp_clear` | OS temp directory cleanup | All |
| `cache_clear` | Browser/app cache clearing | All |
| `registry_clean` | Orphaned registry entry removal | Windows |
| `dedup_remove` | Remove duplicate files (keep newest) | All |
| `log_rotate` | Truncate old log files | All |
| `package_clean` | Clear package manager caches | Linux/macOS |

**Safety Mechanisms:**
- **Quarantine mode:** Move to `.vulturius/quarantine/` instead of deleting
- **Dry-run mode:** Report what would be done without acting
- **Undo journal:** Every action logged to SQLite for reversal
- **System file protection:** Hard-blocked paths never touched
- **Confirmation thresholds:** Large deletions require explicit approval

### 4.4 LLM Engine (Optional)

**Purpose:** Augment analysis for ambiguous cases.

**When LLM is invoked:**
1. File type is unrecognized and heuristics are inconclusive
2. User explicitly requests "AI review" of results
3. Complex dependency analysis (is this DLL used by anything?)
4. Natural language scan queries ("find files I haven't opened in 2 years")

**Architecture:**
```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  File Context │────▶│  LLM Prompt  │────▶│  Structured  │
│  (metadata,   │     │  Engine      │     │  Response    │
│   path, type) │     │  (OpenAI/    │     │  (score,     │
│               │     │   compatible) │     │   reason)    │
└──────────────┘     └──────────────┘     └──────────────┘
```

**Cost Controls:**
- Configurable max LLM calls per scan
- Local model support (Ollama, LM Studio)
- Caching of LLM decisions for identical file patterns
- Budget limit (max $/scan)
- Fallback to heuristic-only if LLM unavailable

**Config:**
```toml
[llm]
enabled = false
provider = "openai"          # openai, ollama, custom
model = "gpt-4o-mini"       # cost-effective for bulk analysis
api_key = ""                # or from env
max_calls_per_scan = 100
timeout_seconds = 30
cache_decisions = true
```

---

## 5. CLI Design

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

Options:
  -v, --verbose...    Increase logging verbosity
  -q, --quiet         Suppress all output except errors
  -o, --output <FMT>  Output format: json, json-pretty, table, list
  -c, --config <PATH> Custom config file path
  --dry-run           Simulate without making changes
  --quarantine        Move to quarantine instead of deleting
```

### Usage Examples

```bash
# Quick scan with intelligent defaults
vulturius scan --quick

# Deep scan with custom parameters
vulturius scan --depth unlimited --exclude "D:\Games" --threads 8

# Scan and immediately analyze
vulturius scan --analyze --risk-tolerance balanced

# Clean everything the analyzer recommends
vulturius clean --from-last-scan --quarantine

# Find unused apps
vulturius app list --unused --older-than 90d

# Uninstall an unused app with leftover cleanup
vulturius app uninstall "OldProgram" --cleanup-leftovers

# Registry cleanup (Windows)
vulturius registry scan --orphaned-entries --unused-keys

# Natural language query (requires LLM)
vulturius scan --query "find large video files I haven't watched"

# Generate a report
vulturius report --format html --output cleanup-report.html

# Undo last cleanup
vulturius undo --last --confirm
```

### Configuration File (`~/.config/vulturius/config.toml`)

```toml
[general]
language = "en"
output_format = "table"
log_level = "info"
max_threads = 0          # 0 = auto-detect

[scan]
default_depth = "unlimited"
default_exclusions = ["%SYSTEMROOT%", "%TEMP%\\System", "Program Files\\Windows*"]
scan_hidden = false
scan_system = false
collect_hashes = false   # expensive, for dedup only
max_scan_size = "0"      # 0 = unlimited

[analysis]
risk_tolerance = "balanced"
unused_app_threshold_days = 90
min_file_age_days = 30
llm_enabled = false

[cleanup]
quarantine_enabled = true
quarantine_path = "~/.local/share/vulturius/quarantine"
auto_backup_before_uninstall = true
confirm_threshold_mb = 1000  # prompt for confirmation above this

[llm]
enabled = false
provider = "openai"
model = "gpt-4o-mini"
api_key = ""
max_calls_per_scan = 50
cache_decisions = true

[schedule]
auto_scan_enabled = false
auto_scan_interval = "7d"
auto_clean = false
```

---

## 6. GUI Design

### Design Philosophy: Ultra Clean FUI (Futuristic User Interface)

**FUI = Futuristic User Interface** — the visual language of sci-fi cinema.

Reference inspirations:
- **Iron Man's J.A.R.V.I.S. HUD** — floating holographic panels, circular gauges, real-time data streams
- **Tron / Tron: Legacy** — grid lines, neon glow trails, light-cycle aesthetics
- **Ex Machina** — clinical white-on-black minimalism, holographic projections
- **Blade Runner 2049** — volumetric light, holographic ads, deep atmospheric depth
- **Arrival (Heptapog UI)** — organic circular interfaces, radial symmetry
- **Cyberpunk 2077** — layered holographic menus, glitch transitions
- **Minority Report** — gesture-driven, multi-panel, floating data cards

Core aesthetic principles:
- **Holographic depth** — layers of glassmorphism, floating panels at varying z-depths, backdrop blur
- **Neon glow accents** — cyan/teal primary glow, amber for warnings, crimson pulses for danger
- **Data viz as primary UI** — donut charts, heatmaps, particle flows, radial gauges replace tables
- **Minimal typography** — monospace data fonts (JetBrains Mono, IBM Plex Mono), oversized numbers, sparse labels
- **Ambient animation** — breathing glows, scanning lines, pulsing indicators (subtle, never distracting)
- **Radial & circular layouts** — dials, concentric rings, orbiting data points over flat grids
- **Transparency everywhere** — frosted glass panels, see-through backgrounds, layered opacity
- **No traditional chrome** — no visible buttons, borders, or scrollbars; hover + gesture-driven
- **Real-time data streams** — live-updating counters, flowing progress, animated number transitions
- **Dark void canvas** — deep black (#000008) background, content floats in empty space
- **Scan-line aesthetic** — subtle CRT/hologram scan-line overlay on data regions
- **Sound design** — subtle UI feedback tones (optional, sci-fi ambiance)

### Layout (FUI Dashboard)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │  [floating glass panel — top bar, barely visible]                   │   │
│   │  VULTURIUS ──── ● ONLINE  ────  ⚙                                  │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│                        ┌──────────────────────────┐                        │
│                        │                          │                        │
│                        │    ╭──────────────╮      │                        │
│   ┌───────────┐        │    │              │      │                        │
│   │           │        │    │   512 GB     │      │                        │
│   │  ◄ NAV   │        │    │   DISK A:    │      │                        │
│   │  RING    │        │    │              │      │                        │
│   │  MENU    │        │    │  ┌───┐  ┌───┐ │      │                        │
│   │  (orbits)│        │    │  │312│  │200│ │      │                        │
│   │           │        │    │  USED│FREE│ │      │                        │
│   └───────────┘        │    │  └───┘  └───┘ │      │                        │
│                        │    │              │      │                        │
│                        │    │  ● 39% used  │      │                        │
│                        │    │  ▲ 23.4 GB   │      │                        │
│                        │    │    reclaimable│      │                        │
│                        │    ╰──────────────╯      │                        │
│                        │         ║ scan-line       │                        │
│                        │         ║ animation       │                        │
│                        └──────────────────────────┘                        │
│                                                                             │
│   ┌────────────┐   ┌────────────┐   ┌────────────┐   ┌────────────┐       │
│   │  ◉ SCANNING│   │  ⚡ SAFE    │   │  ⚠ AT RISK │   │  ☰ RECENT  │       │
│   │  847,293   │   │  TO DELETE │   │  ITEMS     │   │  ACTIONS   │       │
│   │  files     │   │  23.4 GB   │   │  47 items  │   │            │       │
│   │  ━━━━━━━━━│   │  ━━━━━━━━━ │   │  ▁▂▃▅▇▆    │   │  ✓ 2.3 GB  │       │
│   │  67%      │   │  ●●●●●○○○ │   │  ●●●○○○○○ │   │  ✓ 3 apps  │       │
│   └────────────┘   └────────────┘   └────────────┘   └────────────┘       │
│                                                                             │
│   ── STORAGE MAP ──────────────────────────────────────────────────────     │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │  [heatmap grid — directories as cells, color = reclaimable ratio]   │   │
│   │  ░░▒▒▓▓██░░▒▒▓▓██░░▒▒▓▓██░░▒▒▓▓██░░▒▒▓▓██░░▒▒▓▓██░░▒▒▓▓██░░▒▒▓▓██  │   │
│   │  ░░▒▒▓▓██░░▒▒▓▓██░░▒▒▓▓██░░▒▒▓▓██░░▒▒▓▓██░░▒▒▓▓██░░▒▒▓▓██░░▒▒▓▓██  │   │
│   │  ░░▒▒▓▓██░░▒▒▓▓██░░▒▒▓▓██░░▒▒▓▓██░░▒▒▓▓██░░▒▒▓▓██░░▒▒▓▓██░░▒▒▓▓██  │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│   ── QUICK ACTIONS ─────────────────────────────────────────────────────    │
│   [ ◉ SCAN ]    [ ⚡ CLEAN ]    [ 🔍 ANALYZE ]    [ 📊 REPORT ]            │
│   (glow on      (pulse on      (rotate on        (static,              │
│    hover)       hover)         hover)             activate on click)    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Screens/Pages

| Page | FUI Element | Description |
|------|-------------|-------------|
| Dashboard | Central orb + floating panels | Overview, storage orb, quick actions, activity stream |
| Scanner | Radar sweep animation | Configure scans, live file counter, category radar |
| Analyzer | Confidence heatmap | Review recommendations, risk dial, LLM insight cards |
| Cleaner | Particle destruction FX | Execute cleanup, animated progress, undo timeline |
| Apps | Grid of app cards with glow | Installed apps, usage orbit rings, uninstall |
| Registry | Tree with neon branches | Windows-only, orphan detection, key visualization |
| Settings | Minimalist control panel | Configuration, LLM setup, exclusions, scheduling |

### Key UI Components

1. **Central Storage Orb** — 3D-rendered sphere showing disk usage, segments glow by category
2. **Radar Sweep Scanner** — animated radar sweep showing scan progress across directories
3. **Confidence Heatmap** — grid of files colored by safety-to-delete score
4. **Particle Destruction FX** — files "dissolve" into particles when cleaned
5. **Activity Stream** — vertical scrolling log with glow-fade animation
6. **Risk Dial** — circular dial control for adjusting risk tolerance
7. **App Usage Orbit Rings** — concentric rings showing app launch frequency
8. **Scan-line Overlay** — subtle holographic scan-line effect on data regions
9. **Gesture Navigation** — swipe between pages, pinch to zoom details
10. **Sound Feedback** — optional sci-fi UI tones (scan ping, cleanup swoosh, alert chirp)

### Color System (FUI Neon Palette)

```
Void Black:     #000008  — canvas background
Panel Glass:    #0A0A1AFF — floating panels (with backdrop-blur)

Neon Cyan:      #00F0FF  — primary glow, active elements, safe items
Neon Teal:      #00D4AA  — secondary actions, progress
Neon Amber:     #FFB800  — warnings, caution items
Neon Crimson:   #FF2D55  — danger, risky items, critical alerts
Neon Purple:    #B829FF  — LLM/AI features
Neon White:     #E8E8FF  — text, labels, data readouts

Glow Intensity:  box-shadow: 0 0 20px rgba(0, 240, 255, 0.3)
Scan-line:      repeating-linear-gradient(transparent, rgba(0,240,255,0.03))
Glass:          background: rgba(10, 10, 26, 0.65); backdrop-filter: blur(20px)

Typography:
  Primary: JetBrains Mono / IBM Plex Mono (data readouts)
  Secondary: Inter / SF Pro (labels, descriptions)
  Numbers: oversized, tabular-nums, tracking-tight
```

---

## 7. LLM Integration

### Prompt Templates

**File Classification Prompt:**
```
You are a disk cleanup assistant. Analyze this file and determine if it is safe to delete.

File: {path}
Type: {mime_type}
Size: {size}
Last accessed: {last_accessed}
Location: {directory_category}
Context: {surrounding_files_summary}

Respond in JSON:
{
  "safe_to_delete": boolean,
  "confidence": 0.0-1.0,
  "reason": "brief explanation",
  "category": "temp|cache|log|orphan|unknown"
}
```

**Natural Language Query Prompt:**
```
The user wants to find files matching this description on their disk.
Given the file categories and metadata available, generate a scan query.

User query: "{user_query}"
Available filters: [age, size, type, location, name_pattern]

Respond with a structured scan configuration.
```

### Cost Management

| Strategy | Implementation |
|----------|---------------|
| Batch requests | Group 10-50 files per API call |
| Decision caching | Hash file metadata → cache LLM decision |
| Model selection | gpt-4o-mini for bulk, gpt-4o for complex |
| Local fallback | Ollama with small models when offline |
| Budget caps | Configurable $/scan limit, auto-disable |

---

## 8. Phased Implementation

### Phase 1: Foundation (Weeks 1-2)
- [ ] Rust project scaffold with Cargo workspace
- [ ] Platform abstraction layer (traits + platform impls)
- [ ] Basic filesystem scanner (single-threaded)
- [ ] File metadata collection
- [ ] Config system (TOML)
- [ ] Logging infrastructure
- [ ] CLI skeleton with clap
- [ ] Unit tests for core types

### Phase 2: Core Engine (Weeks 3-4)
- [ ] Parallel scanner with tokio
- [ ] File classification engine
- [ ] Analyzer with scoring system
- [ ] Risk tolerance configuration
- [ ] App detection (all platforms)
- [ ] SQLite integration for scan results
- [ ] CLI scan/analyze commands complete
- [ ] Integration tests

### Phase 3: Cleanup Engine (Weeks 5-6)
- [ ] Cleaner engine with safety mechanisms
- [ ] Quarantine system
- [ ] Undo journal
- [ ] App uninstall with leftover cleanup
- [ ] Temp/cache clearing
- [ ] Deduplication detection
- [ ] CLI clean/undo commands
- [ ] E2E tests with test fixtures

### Phase 4: Windows Registry (Week 7)
- [ ] Registry scanner (orphaned entries)
- [ ] Registry cleaner
- [ ] App uninstall registry cleanup
- [ ] Registry backup/restore
- [ ] Windows-specific tests

### Phase 5: LLM Integration (Week 8)
- [ ] LLM engine abstraction
- [ ] OpenAI provider
- [ ] Ollama/local provider
- [ ] Decision caching
- [ ] Natural language query support
- [ ] Cost controls
- [ ] LLM-specific tests

### Phase 6: Tauri GUI (Weeks 9-11)
- [ ] Tauri project setup
- [ ] Svelte 5 + Tailwind CSS scaffold
- [ ] FUI design system / component library (glass panels, neon glow, scan-lines)
- [ ] Three.js integration for 3D orb
- [ ] Dashboard page (central orb, floating panels, heatmap)
- [ ] Scanner page (radar sweep, live progress)
- [ ] Analyzer/Reviewer page (confidence heatmap, risk dial)
- [ ] Cleaner page (particle destruction FX, progress)
- [ ] Apps page (glow cards, orbit rings)
- [ ] Settings page
- [ ] Registry page (Windows-only, neon tree)
- [ ] Sound FX integration (optional)

### Phase 7: Polish & Release (Weeks 12-13)
- [ ] FUI theme refinements (glow intensity, animation timing)
- [ ] Internationalization (i18n)
- [ ] Scheduled scans (systemd launchd, Windows Task Scheduler)
- [ ] Build scripts (cross-compilation)
- [ ] Installer packages (NSIS, DMG, AppImage)
- [ ] Documentation
- [ ] Performance profiling and optimization
- [ ] Security audit
- [ ] Release candidates and beta testing

---

## 9. Testing Strategy

### Test Pyramid

```
        ┌─────────────┐
        │   E2E Tests  │     ~5%  — Full integration scenarios
        ├─────────────┤
        │ Integration │     ~20% — Multi-module interactions
        ├─────────────┤
        │  Unit Tests  │     ~75% — Individual functions/types
        └─────────────┘
```

### Unit Tests
- Every public function in core engines
- Scoring algorithm edge cases
- Config parsing and validation
- File classification accuracy
- Path handling (Unicode, symlinks, special chars)

### Integration Tests
- Scanner → Analyzer → Cleaner pipeline
- SQLite read/write cycles
- LLM integration with mock provider
- Cross-platform adapter behavior

### E2E Tests
- Full scan of test directory tree
- Cleanup with quarantine and undo
- App install → detect → uninstall flow
- Registry write → detect → clean → restore

### Test Infrastructure
```rust
// Test fixtures
tests/fixtures/
├── file_tree/           # Synthetic directory structures
├── registry_hive/       # Windows registry test data
├── app_installs/        # Fake app installations
├── scan_results/        # Expected scan outputs
└── llm_responses/       # Mock LLM responses

// Testing approach
- proptest for fuzzing path handling
- mockall for dependency mocking
- tempdir for isolated filesystem tests
- cargo test --all-features
- CI: GitHub Actions (Ubuntu, macOS, Windows runners)
```

### Performance Benchmarks
```rust
// Criterion benchmarks
- Scan 100K files: target < 30s on modern hardware
- Analyze 100K entries: target < 10s
- Memory usage: < 200MB for 1M file scan
- Concurrency scaling: linear up to 8 threads
```

---

## 10. Project Structure

```
vulture/
├── Cargo.toml                    # Workspace root
├── Cargo.lock
├── PLAN.md                       # This document
├── README.md
├── LICENSE
│
├── crates/
│   ├── vulturius-core/           # Core library
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── scanner/
│   │       │   ├── mod.rs
│   │       │   ├── engine.rs
│   │       │   ├── filter.rs
│   │       │   └── platform.rs
│   │       ├── analyzer/
│   │       │   ├── mod.rs
│   │       │   ├── engine.rs
│   │       │   ├── scorer.rs
│   │       │   ├── classifier.rs
│   │       │   └── app_detector.rs
│   │       ├── cleaner/
│   │       │   ├── mod.rs
│   │       │   ├── engine.rs
│   │       │   ├── quarantine.rs
│   │       │   ├── journal.rs
│   │       │   └── operations.rs
│   │       ├── llm/
│   │       │   ├── mod.rs
│   │       │   ├── engine.rs
│   │       │   ├── provider/
│   │       │   │   ├── mod.rs
│   │       │   │   ├── openai.rs
│   │       │   │   ├── ollama.rs
│   │       │   │   └── mock.rs
│   │       │   └── cache.rs
│   │       ├── platform/
│   │       │   ├── mod.rs
│   │       │   ├── trait.rs
│   │       │   ├── windows.rs
│   │       │   ├── macos.rs
│   │       │   └── linux.rs
│   │       ├── registry/         # Windows-specific
│   │       │   ├── mod.rs
│   │       │   ├── scanner.rs
│   │       │   └── cleaner.rs
│   │       ├── config.rs
│   │       ├── types.rs
│   │       ├── error.rs
│   │       └── db.rs
│   │
│   ├── vulturius-cli/            # CLI binary
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── commands/
│   │       │   ├── mod.rs
│   │       │   ├── scan.rs
│   │       │   ├── analyze.rs
│   │       │   ├── clean.rs
│   │       │   ├── app.rs
│   │       │   ├── registry.rs
│   │       │   ├── quarantine.rs
│   │       │   ├── report.rs
│   │       │   ├── config.rs
│   │       │   ├── history.rs
│   │       │   └── undo.rs
│   │       └── output/
│   │           ├── mod.rs
│   │           ├── table.rs
│   │           ├── json.rs
│   │           └── progress.rs
│   │
│   └── vulturius-gui/            # Tauri app
│       ├── Cargo.toml
│       ├── src/
│       │   ├── main.rs
│       │   └── commands/          # Tauri command handlers
│       │       ├── mod.rs
│       │       ├── scan.rs
│       │       ├── analyze.rs
│       │       ├── clean.rs
│       │       └── system.rs
│       ├── src-tauri/
│       │   ├── tauri.conf.json
│       │   └── capabilities/
│       └── frontend/             # Svelte 5 app
│           ├── package.json
│           ├── svelte.config.js
│           ├── vite.config.ts
│           ├── tailwind.config.ts
│           └── src/
│               ├── main.ts
│               ├── app.svelte
│               ├── lib/
│               │   ├── components/
│               │   │   ├── Orb.svelte          # 3D storage orb
│               │   │   ├── GlassPanel.svelte   # Frosted glass container
│               │   │   ├── NeonButton.svelte   # Glowing action button
│               │   │   ├── RadarSweep.svelte   # Scanner animation
│               │   │   ├── Heatmap.svelte      # Confidence heatmap
│               │   │   ├── ParticleFX.svelte   # Destruction effects
│               │   │   ├── RiskDial.svelte     # Circular risk control
│               │   │   ├── OrbitRing.svelte    # App usage rings
│               │   │   ├── ScanLine.svelte     # CRT overlay effect
│               │   │   └── ActivityStream.svelte
│               │   ├── pages/
│               │   ├── stores/
│               │   ├── services/
│               │   ├── types/
│               │   └── utils/
│               └── styles/
│               │   ├── global.css
│               │   ├── fui.css         # FUI design tokens
│               │   ├── glow.css        # Neon glow effects
│               │   └── animation.css   # Ambient animations
│
├── tests/
│   ├── unit/
│   ├── integration/
│   ├── e2e/
│   └── fixtures/
│
├── scripts/
│   ├── build.sh
│   ├── cross-build.sh
│   └── benchmark.sh
│
├── docs/
│   ├── architecture.md
│   ├── api.md
│   ├── contributing.md
│   └── user-guide.md
│
└── .github/
    └── workflows/
        ├── ci.yml
        ├── release.yml
        └── benchmarks.yml
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

---

*Plan version: 1.1 | Last updated: 2026-05-04*
