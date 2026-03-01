# User Guide: Omega Entry Point (backend/src/main.rs)

## Overview

The `backend/src/main.rs` file is the entry point for the Omega binary. It handles command-line argument parsing, enforces security checks, and orchestrates the startup of the Omega agent. This guide explains how to use Omega and how it starts up internally.

## What is Omega?

Omega is a personal AI agent infrastructure that connects to messaging platforms (Telegram and WhatsApp) and delegates reasoning to configurable AI backends (6 providers, with Claude Code CLI as the default). Think of it as a bridge between your favorite chat platform and your AI assistant.

When you run Omega:
1. It connects to your configured messaging channels (Telegram, WhatsApp, or both)
2. Listens for incoming messages
3. Classifies complexity and routes to the appropriate AI model (Sonnet for simple, Opus for complex)
4. Returns the AI's response back to your chat platform
5. Runs background loops (scheduler, heartbeat, summarizer)

## Getting Started

### Installation Basics

Before using Omega, ensure you have:
- **Rust** installed (for building)
- **Claude CLI** installed and authenticated (`claude` in your PATH)
- **Telegram bot token** (if using Telegram channel)
- A **config.toml** file with your settings

### Configuration

Omega reads a `config.toml` file that specifies:
- Which AI provider to use (e.g., Claude Code CLI)
- Which messaging channels to enable (e.g., Telegram)
- Provider settings (max conversation turns, allowed tools)
- Channel credentials (bot tokens)
- Database location for conversation memory

See `config.example.toml` for a template. Environment variables can override config file settings.

## Available Commands

### 1. omega start
**Purpose:** Launch the Omega agent daemon

```bash
omega start
```

or with a custom config file:

```bash
omega --config /path/to/config.toml start
```

**What happens:**
1. Loads your configuration
2. Initializes the Claude Code provider
3. Connects to enabled channels (e.g., Telegram)
4. Opens the conversation database
5. Runs pre-flight health checks
6. Enters an event loop, listening for messages 24/7
7. Processes each incoming message through Claude
8. Sends responses back to the originating channel

**When to use:** Running the agent continuously. Usually set up as a LaunchAgent on macOS or systemd service on Linux so it starts automatically.

**How it stops:**
- Send a termination signal (Ctrl+C, or kill command)
- Omega gracefully shuts down, completing any in-flight operations
- Clean exit with no data loss (conversation history is in the database)

### 2. omega status
**Purpose:** Check system health and provider availability

```bash
omega status
```

or with a custom config:

```bash
omega --config /path/to/config.toml status
```

**Output example:**
```
Ω Omega — Status Check

Config: config.toml
Default provider: claude-code

  claude-code: available
  telegram: configured
```

**What it checks:**
- Reads your configuration
- Verifies Claude CLI is installed and in PATH
- Confirms Telegram bot token is set (if enabled)
- Reports any issues (e.g., "not found", "missing bot_token")

**When to use:** Troubleshooting or confirming everything is set up before launching the agent.

### 3. omega ask
**Purpose:** Send a one-shot message to Claude without launching the full agent

```bash
omega ask "What is the capital of France?"
```

Multiple-word messages work automatically:

```bash
omega ask What time is it in Tokyo right now?
```

With custom config:

```bash
omega --config /path/to/config.toml ask "Your question here"
```

**What happens:**
1. Loads configuration
2. Initializes the Claude provider
3. Sends your message to Claude
4. Prints Claude's response
5. Exits (doesn't launch the gateway or listen for channel messages)

**When to use:** Quick queries without starting the full agent. Useful for scripts, one-off tasks, or testing that Claude is working.

### 4. omega init
**Purpose:** Interactive setup wizard or non-interactive deployment

**Interactive mode** (default, no deployment args):
```bash
omega init
```

**Non-interactive mode** (when `--telegram-token` or `--allowed-users` is provided):
```bash
# Minimal non-interactive deployment
omega init --telegram-token "123:ABC" --allowed-users "842277204,123456"

# Via environment variables
OMEGA_TELEGRAM_TOKEN="123:ABC" OMEGA_ALLOWED_USERS="842277204" omega init

# Full options
omega init --telegram-token "123:ABC" --allowed-users "842277204" \
  --claude-setup-token "..." --whisper-key "sk-..." \
  --google-credentials ~/client_secret.json --google-email user@gmail.com
```

**Available CLI arguments** (all support `OMEGA_` env var prefix):

| Argument | Env Var | Purpose |
|----------|---------|---------|
| `--telegram-token` | `OMEGA_TELEGRAM_TOKEN` | Telegram bot token |
| `--allowed-users` | `OMEGA_ALLOWED_USERS` | Comma-separated Telegram user IDs |
| `--claude-setup-token` | `OMEGA_CLAUDE_SETUP_TOKEN` | Anthropic setup token for headless auth |
| `--whisper-key` | `OMEGA_WHISPER_KEY` | OpenAI API key for Whisper transcription |
| `--google-credentials` | `OMEGA_GOOGLE_CREDENTIALS` | Path to Google OAuth client_secret.json |
| `--google-email` | `OMEGA_GOOGLE_EMAIL` | Gmail address for Google Workspace |

**What it does:**
- **Interactive:** Guides you through configuration step-by-step with cliclack-styled prompts
- **Non-interactive:** Validates inputs, creates `~/.omega/`, generates `config.toml`, installs service -- all without user interaction
- Creates or updates your `config.toml` file

**When to use:** First-time setup (interactive), automated/scripted deployments (non-interactive), or when reconfiguring from scratch.

### 5. omega pair
**Purpose:** Pair (or re-pair) WhatsApp by scanning a QR code

```bash
omega pair
```

**What happens:**
1. Checks if a WhatsApp session already exists (`~/.omega/whatsapp_session/whatsapp.db`)
2. If already paired, asks whether to re-pair (deletes old session if confirmed)
3. Starts a standalone WhatsApp pairing bot
4. Displays a QR code in the terminal (Unicode half-block characters)
5. Waits up to 60 seconds for the user to scan from WhatsApp → Linked Devices
6. Reports success or failure

**When to use:** Linking WhatsApp to Omega without running the full init wizard. Also useful for re-pairing after unlinking from the phone.

## Global Options

All commands support the `--config` flag:

```bash
omega --config /custom/path/config.toml <COMMAND>
```

Default config path: `config.toml` in the current working directory.

## How Omega Starts (Internal Flow)

### Step 1: Argument Parsing
```
User input: omega start
           ↓
Clap parses: { command: Start, config: "config.toml" }
```

### Step 2: Logging Setup
```
Tracing subscriber initializes with log level from OMEGA_RUST_LOG env var
(defaults to "info" if not set)
```

### Step 3: Security Check
```
Check: Am I running as root?
    ↓
  YES? → STOP. Error: "Must not run as root. Use LaunchAgent instead."

  NO?  → Continue to step 4
```

This security check is crucial because the Claude CLI doesn't allow root execution. If you need Omega to auto-start on your Mac, use a LaunchAgent (~/Library/LaunchAgents/) instead of a LaunchDaemon (/Library/LaunchDaemons/).

### Step 4: Match Command

#### If `omega start`:
```
Load config.toml
    ↓
Deploy bundled prompts (SYSTEM_PROMPT.md, WELCOME.toml) if not present
    ↓
Load prompts from ~/.omega/
    ↓
Deploy bundled skills if not present
    ↓
Ensure projects directory exists (~/.omega/projects/)
    ↓
Load all projects from projects/*/ROLE.md
    ↓
Create workspace directory (~/.omega/workspace/) if not present
    ↓
Build Claude Code provider (with working_dir set to workspace)
    ↓
Check if Claude CLI available
    ├─ Not available? → STOP with error
    └─ Available? Continue
    ↓
Initialize Telegram channel (if enabled)
    ├─ No bot_token? → STOP with error
    ├─ Not enabled? → Skip
    └─ Enabled and configured? Continue
    ↓
Check: At least one channel enabled?
    ├─ No channels? → STOP with error
    └─ Yes? Continue
    ↓
Open SQLite database (~/.omega/data/memory.db)
    ↓
Run health checks (config validity, database schema, provider health)
    ├─ Any check fails? → STOP with error
    └─ All pass? Continue
    ↓
Enter gateway event loop
    ↓
Listen for messages from channels indefinitely
    ↓
For each message:
    ├─ Extract text and sender
    ├─ Check if sender is authorized (auth.allowed_users)
    ├─ Sanitize prompt (remove injections)
    ├─ Fetch conversation history from database
    ├─ Send to Claude with context
    ├─ Get response back
    ├─ Store in database (conversation and audit log)
    └─ Send to channel
```

#### If `omega status`:
```
Load config.toml
    ↓
Print provider name and config path
    ↓
Check Claude CLI availability
    ├─ Available? → Print "available"
    └─ Not available? → Print "not found"
    ↓
Check Telegram configuration
    ├─ Not configured? → Print "not configured"
    ├─ Enabled, no token? → Print "enabled but missing bot_token"
    ├─ Disabled? → Print "disabled"
    └─ Enabled, token set? → Print "configured"
    ↓
Exit
```

#### If `omega ask <MESSAGE>`:
```
Join message words into a single prompt
    ↓
Load config.toml
    ↓
Build Claude Code provider
    ↓
Check if Claude CLI available
    ├─ Not available? → STOP with error
    └─ Available? Continue
    ↓
Create empty context (no history, just the prompt)
    ↓
Send prompt to Claude
    ↓
Print response to console
    ↓
Exit
```

#### If `omega init`:
```
Check: --telegram-token or --allowed-users provided?
    ├─ Yes → Non-interactive deployment (init::run_noninteractive)
    │        ↓
    │   Parse --allowed-users (comma-separated i64 values)
    │        ↓
    │   Create ~/.omega, validate Claude CLI
    │   Apply --claude-setup-token if provided
    │   Register --google-credentials if provided
    │        ↓
    │   Generate config.toml, install service quietly
    │        ↓
    │   Exit
    │
    └─ No  → Interactive setup wizard (init::run)
             ↓
        Guide user through questions (cliclack prompts)
             ↓
        Create config.toml
             ↓
        Exit
```

## Important Security Notes

### Root Prevention
Omega **must not run as root**. The code explicitly checks:
```rust
if unsafe { libc::geteuid() } == 0 {
    error!("Must not run as root");
}
```

**Why?** The Claude CLI itself rejects root execution for security reasons. If you need Omega to auto-start:
- **macOS:** Use LaunchAgent (`~/Library/LaunchAgents/com.omega-cortex.omega.plist`)
- **Linux:** Use systemd user service (not system service)
- **Windows:** Use Task Scheduler under your user account (not system)

### Configuration Secrets
Your `config.toml` contains:
- Telegram bot token (sensitive!)
- Claude Code settings

**Never commit `config.toml` to version control.** It's in `.gitignore`. Use `config.example.toml` as a template instead.

## Provider System

Omega is designed to support multiple AI providers. Currently, **Claude Code CLI** is the only implemented provider (and the default).

When you run `omega start`:
1. The code reads `provider.default` from config (currently only "claude-code" supported)
2. Extracts provider-specific settings (max_turns, allowed_tools, timeout_secs, model, model_complex)
3. Resolves the workspace directory (`~/.omega/workspace/`) and ensures it exists
4. Extracts `model_fast` (from `cc.model`, default `"claude-sonnet-4-6"`) and `model_complex` (from `cc.model_complex`, default `"claude-opus-4-6"`) from the Claude Code config
6. Creates a ClaudeCodeProvider instance by calling `from_config(cc.max_turns, cc.allowed_tools, cc.timeout_secs, Some(workspace_dir), cc.max_resume_attempts, model_fast.clone())`
7. Passes `model_fast` and `model_complex` to the Gateway so it can route between models during classification
8. The provider handles the actual Claude API calls

**Future providers** (planned): Anthropic API, OpenAI, Ollama, OpenRouter. The factory function `build_provider()` makes adding new providers straightforward.

## Troubleshooting

### "Omega must not run as root"
**Problem:** You tried to run `sudo omega start`

**Solution:** Don't use `sudo`. Set up a LaunchAgent instead:
```bash
omega init  # This will create the LaunchAgent for you
```

### "provider 'claude-code' is not available"
**Problem:** The Claude CLI isn't installed or not in your PATH

**Solution:** Install Claude CLI:
```bash
# Check if installed
which claude

# If not found, follow instructions at: https://github.com/anthropics/claude-code
```

### "Telegram is enabled but bot_token is empty"
**Problem:** You set `enabled = true` in Telegram config but didn't provide a bot token

**Solution:** Edit `config.toml` or set env var:
```bash
export TELEGRAM_BOT_TOKEN="your_token_here"
```

### "No channels enabled. Enable at least one channel in config.toml"
**Problem:** You have no channels configured (all disabled or missing settings)

**Solution:** Edit `config.toml` to enable at least one channel:
```toml
[channel.telegram]
enabled = true
bot_token = "your_bot_token_here"
```

### "Self-check failed. Fix the issues above before starting."
**Problem:** Pre-flight validation found issues

**Solution:** Read the error messages above this line in the console output. Common issues:
- Invalid config.toml syntax
- Database permission issues
- Provider not available
- Channel credentials invalid

## Performance & Logging

### Adjust Log Level
Control verbosity via environment variable:

```bash
RUST_LOG=debug omega start   # Very verbose
RUST_LOG=info omega start    # Default (info, warnings, errors)
RUST_LOG=warn omega start    # Only warnings and errors
```

### Database
Conversation history is stored in SQLite at `~/.omega/data/memory.db`. This includes:
- Conversation threads
- Message history
- Extracted facts
- Audit log (every message processed)

The database automatically grows as conversations accumulate. You can inspect it:
```bash
sqlite3 ~/.omega/data/memory.db "SELECT COUNT(*) FROM messages;"
```

## Advanced: Custom Configuration Path

By default, Omega looks for `config.toml` in the current directory. You can override this:

```bash
omega --config ~/.omega/config.toml start
omega --config /etc/omega/config.toml status
```

This is useful if you store config in a different location or want multiple configurations.

### 5. omega service
**Purpose:** Manage the system service (install, uninstall, status)

```bash
omega service install     # Install as LaunchAgent (macOS) or systemd unit (Linux)
omega service uninstall   # Remove the system service
omega service status      # Check if installed and running
```

With custom config:
```bash
omega --config /path/to/config.toml service install
```

**What happens:**
- **install**: Resolves binary and config paths, generates the appropriate service file for your OS, writes and activates it
- **uninstall**: Stops the service, removes the service file
- **status**: Reports whether the service is installed and running

**When to use:** After initial setup (`omega init`) to make Omega start automatically on login and restart on crash.

For full details, see the [service documentation](src-service-rs.md).

## Summary

| Command | Purpose | Use Case |
|---------|---------|----------|
| `omega start` | Launch the agent | Run continuously (via LaunchAgent) |
| `omega status` | Health check | Verify setup before starting |
| `omega ask` | One-shot query | Quick question, scripting |
| `omega init` | Setup wizard / deployment | First-time config (interactive or non-interactive) |
| `omega pair` | WhatsApp QR pairing | Link/re-link WhatsApp |
| `omega service install` | Install system service | Auto-start on login |
| `omega service uninstall` | Remove system service | Clean removal |
| `omega service status` | Check service state | Verify service is running |

**Key Flow:** Parse args → Check root → Load config → Build provider → Initialize channels → Start event loop.

**Remember:** Omega is meant to run in the background listening to your messaging platforms. The "start" command doesn't exit—it blocks indefinitely. Use `omega service install` to set up auto-restart via LaunchAgent or systemd.
