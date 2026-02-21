# Specification: src/main.rs

## File Path
`/Users/isudoajl/ownCloud/Projects/omega/src/main.rs`

## Purpose
The main entry point for the Omega binary. Orchestrates CLI argument parsing, root privilege detection, async runtime initialization, and routes user commands to appropriate handlers (Start, Status, Ask, Init). Implements the top-level command dispatcher and initializes core infrastructure (provider, channels, memory, gateway).

## Dependencies
- **clap** — CLI argument parsing (Parser, Subcommand derive macros)
- **tokio** — Async runtime with #[tokio::main] macro
- **tracing** — Structured logging
- **tracing_subscriber** — Log level filtering via environment
- **anyhow** — Error handling (Result, bail! macro)
- **libc** — FFI for geteuid() root detection
- **std::sync::Arc** — Atomic reference counting for shared ownership
- **std::collections::HashMap** — Channel registry

## Imports
- `crate::claudemd` — Workspace CLAUDE.md maintenance (init + periodic refresh)
- `crate::commands` — Command handlers submodule
- `crate::gateway` — Event loop gateway
- `crate::init` — Interactive setup wizard
- `crate::selfcheck` — Pre-startup health checks
- `crate::service` — OS-aware service management
- `omega_channels::telegram::TelegramChannel` — Telegram integration
- `omega_core::config` — Configuration loading
- `omega_core::context::Context` — Message context wrapper
- `omega_core::traits::Provider` — Provider trait
- `omega_memory::Store` — SQLite memory backend
- `omega_providers::claude_code::ClaudeCodeProvider` — Claude Code CLI provider

## Structs

### `Cli`
**Purpose:** Top-level CLI argument structure using clap derive macros.

**Fields:**
- `command: Commands` — Subcommand to execute (Start, Status, Ask, Init)
- `config: String` — Path to TOML config file (default: "config.toml")

**Attributes:**
- `#[derive(Parser)]` — Enables clap CLI parsing
- `#[command(...)]` — Metadata: name, version, about description

---

### `Commands`
**Purpose:** Enumeration of available CLI subcommands.

**Variants:**
- `Start` — Launch the Omega agent daemon (connects to channels, runs gateway loop)
- `Status` — Health check: verify provider availability and channel configuration
- `Ask { message: Vec<String> }` — One-shot query: send a message and exit (no persistent gateway)
- `Init` — Interactive setup wizard
- `Service { action: ServiceAction }` — Manage the system service (install, uninstall, status)

---

### `ServiceAction`
**Purpose:** Enumeration of service management subcommands.

**Variants:**
- `Install` — Install Omega as a system service
- `Uninstall` — Remove the Omega system service
- `Status` — Check service installation and running status for initial configuration

---

## Functions

### `main() -> anyhow::Result<()>`
**Signature:** `#[tokio::main] async fn main() -> anyhow::Result<()>`

**Purpose:** Async entry point for the Omega binary. Coordinates initialization, root detection, command routing, and error propagation.

**Flow:**
1. Parse CLI arguments via `Cli::parse()`
2. Initialize tracing subscriber with environment-based log level (defaults to "info")
3. **Root Guard:** Check if running as root via `unsafe { libc::geteuid() } == 0`
   - If root, bail with error message directing user to LaunchAgent setup
4. Match on `cli.command`:
   - **Start:** Load config → deploy bundled prompts → load prompts → deploy bundled skills → build provider → verify availability → build channels → initialize memory → run self-checks → start gateway (wrapped in `Arc::new()`)
   - **Status:** Load config → print provider and channel status information
   - **Ask:** Parse message → load config → build provider → create context → invoke provider → print response
   - **Init:** Run interactive setup wizard
   - **Service:** Dispatch to `service::install`, `service::uninstall`, or `service::status` based on `ServiceAction` subcommand
5. Return `Ok(())` on success or propagate errors

**Error Handling:**
- Uses `?` operator to propagate errors from config loading, provider operations, memory initialization
- Custom error messages for specific failures (root execution, missing bot token, no enabled channels, provider unavailable)
- `anyhow::bail!()` for critical errors that halt execution

**Root Detection Guard:**
```rust
if unsafe { libc::geteuid() } == 0 {
    anyhow::bail!("Omega must not run as root...");
}
```
This is the only unsafe code in main.rs. It prevents Omega from running with elevated privileges (Claude CLI rejects root execution). Directs users to LaunchAgent (user-level) rather than LaunchDaemon (system-level).

---

### `build_provider(cfg: &config::Config, workspace_path: Option<PathBuf>) -> anyhow::Result<Box<dyn Provider>>`
**Purpose:** Factory function to instantiate the configured provider from config.

**Parameters:**
- `cfg: &config::Config` — Parsed configuration object
- `workspace_path: Option<PathBuf>` — Optional workspace directory path passed to the provider for sandbox confinement

**Returns:**
- `anyhow::Result<Box<dyn Provider>>` — Trait object or error

**Logic:**
1. Match on `cfg.provider.default` (string key from config)
2. **"claude-code" case:**
   - Clone Claude Code provider config (or use defaults)
   - Extract `max_turns`, `allowed_tools`, `timeout_secs`, `max_resume_attempts`, and `model` settings
   - Construct `ClaudeCodeProvider::from_config(cc.max_turns, cc.allowed_tools, cc.timeout_secs, workspace_path, cc.max_resume_attempts, cc.model)`
   - Return boxed trait object
3. **Any other provider name:** bail with "unsupported provider" error

**Note:** Extensible pattern for adding new providers (Anthropic, OpenAI, Ollama, OpenRouter).

---

## CLI Argument Parsing

### Clap Configuration
- **Command Name:** "omega"
- **Version:** Sourced from Cargo.toml (version field)
- **About:** "Ω Omega — Personal AI Agent Infrastructure"

### Global Options
- `-c, --config <CONFIG>` — Config file path (default: "config.toml")

### Subcommands

**`omega start`**
- No arguments
- Launches persistent agent with gateway event loop
- Reads config, initializes provider, channels, memory
- Blocks indefinitely (until signal or error)

**`omega status`**
- No arguments
- Prints provider name, Claude Code CLI availability, channel status
- Non-blocking, exits after printing

**`omega ask <MESSAGE>`**
- `<MESSAGE>` — Variable-length trailing arguments (one or more words)
- Concatenates all words into a single prompt
- Sends to provider and prints response
- Non-blocking, exits after receiving response

**`omega init`**
- No arguments
- Launches interactive setup wizard
- Guides user through config creation

**`omega service install`**
- No arguments
- Installs Omega as a macOS LaunchAgent or Linux systemd user unit
- Resolves binary and config paths automatically

**`omega service uninstall`**
- No arguments
- Stops and removes the system service file

**`omega service status`**
- No arguments
- Reports whether service is installed and running

---

## Async Runtime Setup

### Tokio Configuration
- **Macro:** `#[tokio::main]`
- **Effect:** Generates runtime initialization boilerplate
- **Runtime Type:** Multi-threaded async runtime (default)
- **Executor:** Handles all async operations (provider calls, channel I/O, gateway loop)

**Key async operations in main:**
- Provider availability checks: `provider.is_available().await`
- Memory initialization: `Store::new(&cfg.memory).await?`
- Self-checks: `selfcheck::run(&cfg, &memory).await`
- Gateway event loop: `Arc::new(gw).run().await?`
- Provider completion: `provider.complete(&context).await?`
- Claude Code CLI check: `ClaudeCodeProvider::check_cli().await`

---

## Startup Flow (Commands::Start)

1. **Load configuration**
   - Read TOML file from `cli.config` path
   - Parse environment variable overrides
   - Return error if file missing or invalid

1b. **Deploy bundled prompts**
   - Call `config::install_bundled_prompts(&cfg.omega.data_dir)`
   - Writes `SYSTEM_PROMPT.md` and `WELCOME.toml` to `data_dir` on first run
   - Never overwrites existing files (preserves user edits)
   - Then `Prompts::load()` picks up the freshly deployed files

1c. **Workspace directory creation**
   - Resolve the expanded `data_dir` path (e.g., `~/.omega`)
   - Create `{data_dir}/workspace/` directory if it does not exist via `std::fs::create_dir_all()`
   - This directory serves as the sandbox working directory for the provider
   - Compute `workspace_path` as `Option<PathBuf>` for passing to `build_provider()`

1d. **Sandbox mode logging**
   - Read `cfg.sandbox.mode` to determine the active sandbox mode
   - Log the sandbox mode at INFO level (e.g., `"Sandbox mode: sandbox"`)
   - Compute `sandbox_prompt` via `cfg.sandbox.mode.prompt_constraint(&workspace_path_str)` for injection into the gateway

2. **Build provider**
   - Call `build_provider(&cfg, workspace_path)` to instantiate with optional workspace directory
   - Wrap in Arc (atomic reference counting) for thread-safe sharing

3. **Verify provider availability**
   - Call `provider.is_available().await`
   - For Claude Code: checks if `claude` binary exists in PATH
   - Bail if unavailable (user probably hasn't installed Claude CLI)

4. **Build channels**
   - Initialize HashMap<String, Arc<dyn Channel>>
   - If Telegram enabled in config:
     - Verify `bot_token` is not empty
     - Create TelegramChannel instance
     - Insert into map with key "telegram"
   - Bail if no channels enabled (must have at least one)

5. **Initialize memory**
   - Create Store instance with config settings
   - Opens SQLite database at `~/.omega/data/memory.db`
   - Creates tables if first run

6. **Run self-checks**
   - Call `selfcheck::run(&cfg, &memory).await`
   - Verifies: config validity, database schema, provider health, channel credentials
   - Bails if any check fails

7. **Ensure projects directory**
   - Call `omega_skills::ensure_projects_dir(&cfg.omega.data_dir)` to create `~/.omega/projects/` if missing.
   - Projects are hot-reloaded per message in the gateway, not loaded at startup.

8. **Start gateway**
   - Create Gateway instance with provider, channels, memory, auth, channel config, projects, sandbox mode display name, sandbox prompt, `model_fast`, and `model_complex`
   - Wrap gateway in `Arc::new()` for shared ownership across spawned tasks
   - Call `gw.run().await?` to enter event loop (method takes `self: Arc<Self>`)
   - Blocks indefinitely processing messages from channels
   - Terminates on signal (graceful shutdown) or error

---

## Error Handling Patterns

### Result Type
All fallible operations return `anyhow::Result<T>` (alias for `Result<T, anyhow::Error>`).

### Error Propagation
- **`?` operator:** Short-circuits on error, unwraps success value
- **`anyhow::bail!(msg)`:** Creates error with formatted message and returns

### Specific Error Messages
Provides user-friendly context for common failures:
- Root execution → directs to LaunchAgent setup
- Missing bot token → tells user where to configure
- No enabled channels → instructs to enable at least one in config.toml
- Provider unavailable → suggests checking CLI installation
- Self-check failure → directs to fix issues and retry

### Graceful Degradation
Status command doesn't fail if provider unavailable—just reports status.
Ask command fails if provider unavailable (one-shot, no fallback).
Start command requires both provider and at least one channel.

---

## Key Design Patterns

### Arc<dyn Trait> Pattern
```rust
let provider: Arc<dyn omega_core::traits::Provider> = Arc::from(build_provider(&cfg)?);
let mut channels: HashMap<String, Arc<dyn omega_core::traits::Channel>> = HashMap::new();
```
Enables thread-safe shared ownership across async tasks. Each channel and provider task can clone Arc without copying underlying data.

### Factory Function
`build_provider()` encapsulates provider instantiation logic, supporting multiple provider types via match statement. Easily extensible for new providers.

### Config-Driven Initialization
Provider selection, channel configuration, and memory settings all sourced from TOML config file. Environment variables can override. No hardcoded defaults except for config file path.

### Lazy Validation
Root check, provider availability, channel credentials, and self-checks all happen at startup (not at binary build time). Enables clear error messages to users.

---

## Module Dependencies
- `commands` — Currently unused in main.rs (defined but not referenced)
- `gateway` — Core event loop orchestrator
- `init` — Setup wizard
- `selfcheck` — Pre-flight verification
- External crates provide: CLI parsing, async runtime, logging, error handling, platform FFI

---

## Configuration Integration

### Config Loading
```rust
let cfg = config::load(&cli.config)?;
```
Loads TOML from file, merges environment variable overrides.

### Provider Config
```rust
let cc = cfg.provider.claude_code.as_ref().cloned().unwrap_or_default();
let model_fast = cc.model.clone();
let model_complex = cc.model_complex.clone();
ClaudeCodeProvider::from_config(cc.max_turns, cc.allowed_tools, cc.timeout_secs, workspace_path, cc.max_resume_attempts, cc.model)
```
Extracts provider-specific settings (including `timeout_secs`, `max_resume_attempts`, `model`, and `model_complex`) and passes the workspace path for sandbox confinement; provides defaults if not specified. The `model_fast` and `model_complex` values are extracted from the config before building the provider, then passed to `Gateway::new()` for model routing.

### Channel Config
```rust
if let Some(ref tg) = cfg.channel.telegram {
    if tg.enabled { ... }
}
```
Checks if channel configured, then if enabled, then validates required fields (bot_token).

### Memory Config
```rust
let memory = Store::new(&cfg.memory).await?;
```
Passes config to memory store (database path, schema version, etc.).

---

## Summary Table

| Component | Type | Purpose |
|-----------|------|---------|
| `Cli` | Struct | Argument parser definition |
| `Commands` | Enum | Command variants (Start, Status, Ask, Init) |
| `main()` | Async Fn | Entry point, orchestrator |
| `build_provider()` | Fn | Provider factory |
| Root Guard | Check | Prevents execution as root (unsafe libc call) |
| Tracing Init | Logger | Structured logging setup |
| Gateway Loop | Async | Event processor for Start command |
| Config Loading | I/O | TOML parsing and env merge |
| Error Handling | Pattern | anyhow::Result, ? operator, bail! |
