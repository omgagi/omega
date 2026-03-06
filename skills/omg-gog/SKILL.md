---
name = "omg-gog"
description = "Google Workspace CLI — Gmail, Calendar, Drive, Docs, Sheets, Slides, Forms, Chat, Tasks, Contacts and more."
requires = ["omg-gog"]
trigger = "email|gmail|calendar|drive|docs|sheets|slides|forms|chat|tasks|contacts|google|correo|calendario|agenda|contactos|kontakte|kalender|почта|календарь|контакты|e-mail|courrier|courriel|calendrier"
---

# omega-google (omg-gog) — Functionalities Skill

Reference skill for the `omega-google` CLI tool (`omg-gog`). For detailed per-service functionalities, see [SKILL-INDEX.md](SKILL-INDEX.md).

## Installation

### Quick Install (recommended)

Downloads the latest pre-built binary for your OS/arch:

```bash
curl -fsSL https://omgagi.ai/tools/omg-gog/install.sh | sh
```

To install a specific version:

```bash
VERSION=v0.5.0 curl -fsSL https://omgagi.ai/tools/omg-gog/install.sh | sh
```

By default installs to `/usr/local/bin`. Override with `INSTALL_DIR`:

```bash
INSTALL_DIR=~/.local/bin curl -fsSL https://omgagi.ai/tools/omg-gog/install.sh | sh
```

### Build from Source (fallback)

If the installer fails (unsupported platform, network issues, missing release binary), build from source:

```bash
# Clone
git clone https://github.com/omgagi/omg-gog.git ~/builds/omg-gog
cd ~/builds/omg-gog/omg-gog

# Build release binary
cargo build --release

# Install to PATH
cp target/release/omg-gog /usr/local/bin/
# or: sudo cp target/release/omg-gog /usr/local/bin/
```

Requires Rust toolchain (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`).

### Verify

```bash
omg-gog --help
```

---

## Quick Stats

| Metric | Count |
|--------|-------|
| Google API Services | 15 |
| Top-Level CLI Commands | 25 |
| CLI Handler Functions | ~220 |
| Service Functions (URL/body builders) | ~316 |
| Serde Types (structs/enums) | ~218 |
| Infrastructure Modules | 9 |
| Total Tests | ~1,964 |
| Total Source Files | ~98 |

---

## Services Overview

| Service | Command | Alias | Summary |
|---------|---------|-------|---------|
| Auth | `omg-gog auth` | — | OAuth 2.0 flows (desktop/manual/remote/web), token storage, multi-account, aliasing |
| Config | `omg-gog config` | — | JSON5 config file management |
| Gmail | `omg-gog gmail` | — | Threads, messages, labels, drafts, settings, watch (Pub/Sub push), batch |
| Calendar | `omg-gog calendar` | `cal` | Events, calendars, freebusy, RSVP, conflicts, special events, watch (push notifications) |
| Drive | `omg-gog drive` | — | Files, upload/download, permissions, comments, shared drives, watch (change notifications) |
| Docs | `omg-gog docs` | `doc` | Content, export, editing, sed-like regex, markdown, comments |
| Sheets | `omg-gog sheets` | `sheet` | Cell read/write, append, insert, format, notes, export |
| Slides | `omg-gog slides` | `slide` | Presentations, slides, notes, export, markdown-to-slides |
| Forms | `omg-gog forms` | `form` | Form metadata, creation, responses |
| Chat | `omg-gog chat` | — | Spaces, messages, threads, direct messages |
| Classroom | `omg-gog classroom` | `class` | Courses, roster, coursework, materials, submissions (~60+ commands) |
| Tasks | `omg-gog tasks` | `task` | Task lists, task CRUD, done/undo, clear |
| Contacts | `omg-gog contacts` | `contact` | Contact CRUD, search, directory, other contacts |
| People | `omg-gog people` | `person` | Profile retrieval, search, relationships |
| Groups | `omg-gog groups` | `group` | Group listing, membership |
| Keep | `omg-gog keep` | — | Notes listing, search, attachments |
| Apps Script | `omg-gog appscript` | `script`, `apps-script` | Project metadata, source files, function execution |
| Webhook | `omg-gog webhook` | — | HTTP server for testing Google push notification webhooks |

---

## Global Flags

| Flag | Short | Description |
|------|-------|-------------|
| `--json` | `-j` | Output JSON |
| `--plain` | `-p` | Plain tab-separated |
| `--csv` | — | Output CSV |
| `--account` | `-a` | Account email or alias |
| `--verbose` | `-v` | Verbose output |
| `--dry-run` | `-n` | Dry run mode |
| `--force` | `-y` | Skip confirmations |
| `--select` | — | Select specific fields (dot-path) |
| `--results-only` | — | Strip envelope, return primary result |

---

## Architecture

```
main.rs -> cli::execute() -> cli::root::Cli (clap)
  -> cli::dispatch_command()
    -> services::bootstrap_service_context()
    -> handle_* functions
      -> services::{service}::build_*_url() / build_*_body()
      -> http::api::api_get/post/patch/delete()
      -> ServiceContext.write_output()
```

## Source Layout

```
src/
  main.rs                   -- Entry point
  lib.rs                    -- Module tree
  cli/                      -- CLI parsing + dispatch + handlers
  auth/                     -- OAuth, tokens, credential stores
  config/                   -- Config file management
  http/                     -- HTTP client, retry, circuit breaker
  output/                   -- JSON/Plain/CSV/Text formatting
  ui/                       -- Color, progress, prompts
  error/                    -- Exit codes, error types
  time/                     -- Date/time parsing
  services/                 -- Google API service implementations
```

## Key Patterns

- **Serde types**: `#[serde(rename_all = "camelCase")]`, `#[serde(flatten)] pub extra: HashMap<String, Value>`, `#[serde(default)]` on Vec fields
- **URL builders**: standalone `fn build_*_url() -> String`, percent-encoding for path segments
- **Body builders**: return `serde_json::Value`
- **CLI**: clap derive with `#[command(subcommand)]`, aliases via `#[command(alias = "...")]`
- **Tests**: inline `#[cfg(test)]` with `// REQ-{SERVICE}-{NNN}` traceability comments
- **Dispatch**: pattern match in `dispatch_command()`, handler functions return `i32` exit codes

---

## Command Output Filtering Best Practices

When running build/test/nix commands, reduce noise to avoid context pollution:

```bash
# Redirect output, then filter
build_or_test_command > /tmp/output.log 2>&1

# Test filtering — key results only
test_command 2>&1 | grep -i "pass\|fail\|error\|success\|warning" | sort -u | head -20

# Test summary extraction
test_command 2>&1 | grep -E "^\+[0-9]+|-[0-9]+|passed|failed|Some tests" | tail -5

# Build filtering — errors and warnings only
build_command 2>&1 | grep -i "error\|warning\|failed\|success" | head -10

# Order-preserving deduplication
command 2>&1 | grep -i "keyword1\|keyword2" | awk '!seen[$0]++' | head -15

# Case-insensitive keyword filtering
command 2>&1 | grep -i "pass\|fail\|error" | head -20

# Limit output volume
command 2>&1 | head -n 20   # first 20 lines
command 2>&1 | tail -n 10   # last 10 lines
```
