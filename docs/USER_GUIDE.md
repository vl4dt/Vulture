# Vulturius — User Guide

## Installation

### From Source
```bash
git clone https://github.com/vulturius/vulturius.git
cd vulturius
cargo build --release
```

The binary will be at `target/release/vulturius-cli`.

### From Release
Download the appropriate archive for your platform from [GitHub Releases](https://github.com/vulturius/vulturius/releases):
- **Linux**: `vulturius-linux-x86_64.tar.gz`
- **Windows**: `vulturius-windows-x86_64.zip`
- **macOS (Intel)**: `vulturius-macos-x86_64.tar.gz`
- **macOS (Apple Silicon)**: `vulturius-macos-aarch64.tar.gz`

## Quick Start

### 1. Scan Your System
```bash
vulturius-cli scan --path /home/user --depth 10
```

### 2. Analyze Results
```bash
vulturius-cli analyze --scan-id <SCAN_ID>
```

### 3. Review & Clean
```bash
vulturius-cli clean --scan-id <SCAN_ID> --mode dry-run   # Preview
vulturius-cli clean --scan-id <SCAN_ID> --mode quarantine # Safe cleanup
```

## Configuration

Create `~/.config/vulturius/config.toml`:

```toml
[general]
language = "en"
output_format = "table"
log_level = "info"
max_threads = 0        # 0 = auto-detect

[scan]
default_depth = "unlimited"
scan_hidden = false
scan_system = false
collect_hashes = false
max_scan_size = 0      # 0 = unlimited

[analysis]
risk_tolerance = "balanced"    # paranoid, conservative, balanced, aggressive, nuclear
unused_app_threshold_days = 90
min_file_age_days = 30

[cleanup]
quarantine_enabled = true
quarantine_path = "~/.local/share/vulturius/quarantine"
auto_backup_before_uninstall = true
confirm_threshold_mb = 1000

[llm]
enabled = false
provider = "openai"
model = "gpt-4o-mini"
max_calls_per_scan = 50
cache_decisions = true

[schedule]
auto_scan_enabled = false
auto_scan_interval = "7d"
auto_clean = false
auto_clean_interval = "7d"
run_on_startup = false
```

## CLI Commands

### Scan
```bash
# Basic scan
vulturius-cli scan --path /home/user

# Scan with exclusions
vulturius-cli scan --path /home/user --exclude "Downloads" --exclude ".cache"

# Scan with hash collection
vulturius-cli scan --path /home/user --collect-hashes

# Scan multiple targets
vulturius-cli scan --path /home/user --path /var/tmp
```

### Analyze
```bash
# Analyze latest scan
vulturius-cli analyze

# Analyze with specific risk tolerance
vulturius-cli analyze --risk-tolerance aggressive

# Enable LLM analysis
vulturius-cli analyze --llm-enabled
```

### Clean
```bash
# Dry run (preview only)
vulturius-cli clean --mode dry-run

# Quarantine mode (safe, reversible)
vulturius-cli clean --mode quarantine

# Delete mode (permanent)
vulturius-cli clean --mode delete

# Delete with confirmation threshold
vulturius-cli clean --mode delete --confirm-threshold 500
```

### Reports
```bash
# Generate HTML report
vulturius-cli report --format html --output report.html

# Generate JSON report
vulturius-cli report --format json --output report.json

# Generate text report
vulturius-cli report --format text
```

### Apps
```bash
# List installed applications
vulturius-cli apps list

# Detect unused applications
vulturius-cli apps unused --threshold 90
```

### Registry (Windows only)
```bash
# Scan registry for orphans
vulturius-cli registry scan

# Clean orphaned registry entries
vulturius-cli registry clean
```

### Schedule
```bash
# Start auto-scheduler
vulturius-cli schedule start

# Stop auto-scheduler
vulturius-cli schedule stop

# List scheduled tasks
vulturius-cli schedule list
```

## GUI (Tauri Desktop App)

The desktop app provides a visual interface with:

- **Dashboard**: System overview with cleanup recommendations
- **Scanner**: Configure and run filesystem scans
- **Analyzer**: Review analysis results with confidence scores
- **Cleaner**: Execute cleanup operations safely
- **Apps**: Manage installed applications
- **Settings**: Configure all options

## Safety Features

### Quarantine
Files are moved to an isolated directory before deletion, allowing recovery.

### Undo Journal
All cleanup operations are logged and can be reversed.

### System Path Protection
System directories are excluded by default. Enable `scan_system` only when needed.

### Risk Tolerance
Adjust how aggressively files are flagged for cleanup:
- **Paranoid**: Only obvious waste (temp files, crash dumps)
- **Conservative**: Safe cleanup with minimal risk
- **Balanced**: Default recommendation
- **Aggressive**: More cleanup, some risk
- **Nuclear**: Flags everything, manual review required

### Security
- **Path sandboxing**: Operations restricted to allowed directories
- **Permission model**: Read-only, user, and admin privilege levels
- **Audit logging**: All operations recorded for accountability
- **Path traversal protection**: Prevents directory escape attacks

## Incremental Scanning

Vulturius supports incremental scanning to speed up repeated scans:
- Directories are tracked by modification time
- Unchanged directories are skipped entirely
- Snapshot age threshold: 24 hours (configurable)

## LLM Integration

Optional LLM analysis for intelligent file classification:
- **OpenAI**: GPT-4o, GPT-4o-mini
- **Anthropic**: Claude 3 Opus/Sonnet/Haiku
- **Ollama**: Local models

Enable in config:
```toml
[llm]
enabled = true
provider = "openai"
model = "gpt-4o-mini"
api_key = "sk-..."
```

## Troubleshooting

### Logs
Logs are written to `~/.config/vulturius/vulturius.log`.

### Common Issues

**Permission denied**: Run with elevated privileges or check path sandboxing config.

**Scan too slow**: Reduce `max_threads` or use incremental scanning.

**Database locked**: WAL mode should handle concurrent access. If issues persist, restart.

**LLM timeout**: Increase `timeout_seconds` in LLM config.

## Keyboard Shortcuts (GUI)

| Shortcut | Action |
|----------|--------|
| Ctrl+S | Start scan |
| Ctrl+A | Analyze |
| Ctrl+C | Clean |
| Ctrl+R | Generate report |
| Escape | Cancel operation |
| Ctrl+Q | Quit |
