# src/init.rs — Init Wizard Specification

## Path
`src/init.rs`

## Purpose
Interactive setup wizard for new Omega users. Provides a 2-minute guided onboarding experience that creates the data directory structure, validates Claude CLI availability, collects Telegram credentials, generates the configuration file, and provides next steps for running Omega. This is the entry point for `omega init` command.

## Module Overview
The `init.rs` module contains:
- `run()` — Main wizard orchestration function
- `prompt()` — Interactive stdin reader for user input
- Uses `omega_core::shellexpand()` for home directory expansion (imported, not local)

## Init Wizard Flow

### Phase 1: Welcome Banner
**Output:**
```
  Omega — Setup Wizard
  ====================
```

**Purpose:** Creates visual separation and immediately signals to user that they're in an interactive setup experience.

**Duration:** Instant

---

### Phase 2: Data Directory Setup
**Action:** Create `~/.omega` if it doesn't exist

**Logic Flow:**
1. Expand `~/` to actual home directory path using `shellexpand()`
2. Check if directory exists with `Path::new(&data_dir).exists()`
3. If missing: Create recursively with `std::fs::create_dir_all(&data_dir)?`
4. If exists: Confirm to user that directory was already present

**Outputs:**
- Success: `Created ~/.omega`
- Exists: `~/.omega already exists`

**Error Handling:** Propagate `io::Error` via `?` operator to caller

**Purpose:** Ensures persistent storage location exists before later configuration steps. Directory is required for:
- SQLite database (`memory.db`)
- Log files (`omega.log`)
- Service files

---

### Phase 3: Claude CLI Validation
**Action:** Verify `claude` CLI is installed and accessible in PATH

**Logic Flow:**
1. Print non-terminated prompt: `  Checking claude CLI... `
2. Flush stdout to show prompt immediately
3. Execute `claude --version` as subprocess
4. Capture exit status (not output)
5. Check if status is success (`exit_code == 0`)
6. If found: Print `found`
7. If NOT found:
   - Print `NOT FOUND`
   - Show installation instructions: `npm install -g @anthropic-ai/claude-code`
   - Instruct user to re-run `omega init` after installation
   - Return early with `Ok(())` (not an error; wizard pauses gracefully)

**Outputs (Positive Path):**
```
  Checking claude CLI... found
```

**Outputs (Negative Path):**
```
  Checking claude CLI... NOT FOUND

  Install claude CLI first:
    npm install -g @anthropic-ai/claude-code

  Then run 'omega init' again.
```

**Error Handling:** Non-error early return. The wizard exits gracefully without creating config if Claude CLI is missing. This prevents creating a broken configuration.

**Critical Detail:** Uses `.unwrap_or(false)` to gracefully handle execution failures (e.g., `claude` not in PATH), converting them to `false` instead of panicking.

**Purpose:** Guards against misconfiguration. Users cannot proceed without Claude CLI since it's the default (and only currently enabled) provider.

---

### Phase 4: Telegram Bot Token Collection
**Section Header Output:**
```
  Telegram Bot Setup
  ------------------
  Create a bot with @BotFather on Telegram, then paste the token.
```

**Action:** Prompt user for Telegram bot token via stdin

**Logic Flow:**
1. Call `prompt("  Bot token: ")?`
2. Receive trimmed string (empty string if user just presses Enter)
3. Check if empty with `.is_empty()`
4. If empty: Print skip message, set `bot_token = ""`
5. If provided: Store as `bot_token`

**Conditional Output (if empty):**
```
  Skipping Telegram setup.
  You can add it later in config.toml.
```

**Error Handling:** I/O errors propagated via `?` operator

**Validation:** No format validation of token at this stage. Invalid tokens are caught later when bot attempts to connect.

**Purpose:** Collects the core credential needed for Telegram channel integration. Optional because:
- User may want to test Omega locally first
- User may prefer different messaging platform
- Token can be added to config.toml manually later

---

### Phase 5: Telegram User ID Collection (Conditional)
**Condition:** Only asked if `bot_token` is NOT empty

**Section Output:**
```
  Your Telegram user ID (send /start to @userinfobot to find it).
  Leave blank to allow all users.
```

**Action:** Prompt user for Telegram user ID

**Logic Flow:**
1. Call `prompt("  User ID: ")?` only if bot_token is non-empty
2. Attempt to parse as `i64`
3. If parse succeeds: Store `Some(user_id)`
4. If parse fails or blank: Store `None`

**Parse Result Options:**
- `"123456789"` → `Some(123456789)`
- `""` (empty) → `None`
- `"invalid"` (non-numeric) → `None` (no error shown to user)
- `"999999999999999999"` (overflow) → `None`

**Skip Output:**
```
  Leave blank to allow all users.
```

**Purpose:** Enables optional auth filtering. User can:
- Whitelist themselves only: `allowed_users = [123456789]`
- Allow all users who know the token: `allowed_users = []`

The second option is useful for shared bots or group deployments.

---

### Phase 6: Config File Generation
**Action:** Create or skip `config.toml` based on existing file

**Location:** Current working directory, file named `config.toml`

**Logic Flow:**
1. Check if `config.toml` already exists
2. If exists: Skip generation, inform user
3. If missing: Generate fresh config with collected parameters

**Skip Output (if exists):**
```
  config.toml already exists — skipping generation.
  Delete it and run 'omega init' again to regenerate.
```

**Configuration Template Structure:**

```toml
[omega]
name = "Omega"
data_dir = "~/.omega"
log_level = "info"

[auth]
enabled = true

[provider]
default = "claude-code"

[provider.claude-code]
enabled = true
max_turns = 10
allowed_tools = ["Bash", "Read", "Write", "Edit"]

[channel.telegram]
enabled = {telegram_enabled}
bot_token = "{bot_token}"
allowed_users = {allowed_users}

[memory]
backend = "sqlite"
db_path = "~/.omega/memory.db"
max_context_messages = 50
```

**Parameter Substitution:**
- `{telegram_enabled}` → `"true"` if bot_token provided, else `"false"`
- `{bot_token}` → Collected token string (empty if skipped)
- `{allowed_users}` → JSON array format:
  - If user_id provided: `[123456789]`
  - If empty: `[]`

**Default Values Applied:**
- Log level: `info`
- Provider default: `claude-code`
- Max turns per provider: `10`
- Allowed tools: `["Bash", "Read", "Write", "Edit"]` (pre-selected safe tools)
- Memory backend: `sqlite`
- Max context messages: `50`
- Auth: Enabled

**Error Handling:** File write errors propagated via `?` operator. File write failure aborts wizard.

**Output (success):**
```
  Generated config.toml
```

**Important:** Config file is written to CWD, not to `~/.omega`. User should run `omega init` from project root or explicitly copy config to the right location.

---

### Phase 7: Summary and Next Steps
**Output:**
```
  Setup Complete
  ==============

  Next steps:
    1. Review config.toml
    2. Run: omega start
    3. Send a message to your bot on Telegram
```

**Purpose:**
- Confirms successful wizard completion
- Provides explicit next steps to reduce user confusion
- Hints at the post-setup workflow

---

## Interactive Prompts

### `prompt()` Function Specification

**Signature:**
```rust
fn prompt(msg: &str) -> anyhow::Result<String>
```

**Behavior:**
1. Print the message without newline: `print!("{msg}")`
2. Flush stdout to ensure prompt appears immediately (important for interactive UX)
3. Lock stdin: `io::stdin().lock()`
4. Read one line including newline character
5. Trim whitespace (leading, trailing, newlines)
6. Return trimmed string

**Error Handling:** I/O errors become `anyhow::Result<String>` via the `?` operator. Errors bubble up to caller (`run()`).

**Return Values:**
- Normal text input: `"user input"` (trimmed)
- Just pressing Enter: `""` (empty string)
- I/O error: `Err(anyhow::Error)` from io::Result

**Why Flush is Important:** Without `io::stdout().flush()`, the prompt may not appear before the program waits for input, creating a confusing UX where the user doesn't see the prompt they're expected to respond to.

---

### `shellexpand()` (imported from `omega_core`)

The `shellexpand()` utility is imported from `omega_core::shellexpand` (defined in `omega_core::config`). It expands `~/` prefix to `$HOME/`. This module no longer defines its own copy.

---

## Configuration File Generation Details

### Template Design Rationale

**`[omega]` Section:**
- `name` — Used for display/logging. Default is "Omega".
- `data_dir` — Persistent storage location. Pre-configured to `~/.omega`.
- `log_level` — Defaults to `info`. User can change to `debug` for troubleshooting.

**`[auth]` Section:**
- `enabled = true` — Enforces auth checks. Per CLAUDE.md security constraints, auth is always on.

**`[provider]` Section:**
- `default` — Specifies which provider backend to use. Hardcoded to `claude-code` (the only fully-implemented provider).

**`[provider.claude-code]` Section:**
- `enabled` — Must be true since it's the default.
- `max_turns` — Safety limit on Claude Code provider context window. Set to 10 to prevent runaway conversations.
- `allowed_tools` — Whitelist of tools Claude Code can use. Restricted to safe subset:
  - `Bash` — Execute commands
  - `Read` — Read files
  - `Write` — Write files
  - `Edit` — Edit files
  - **Excluded:** `Skill` (plugin system), others for safety

**`[channel.telegram]` Section:**
- `enabled` — Boolean, set based on whether user provided token
- `bot_token` — Sensitive! Should be environment variable in production, but template allows inline for simplicity
- `allowed_users` — List of Telegram user IDs allowed to send messages. Empty allows all.

**`[memory]` Section:**
- `backend` — Hardcoded to `sqlite` (only option currently)
- `db_path` — Where SQLite database is stored. Typically `~/.omega/memory.db`.
- `max_context_messages` — How many historical messages to include in prompt context. Set to 50 as a balance between context and token cost.

---

## Validation Steps

### Pre-Setup Validation
1. **Claude CLI Check** — Only critical validation. Wizard exits gracefully if missing.

### Post-Setup Validation (Not in `init.rs`, but in downstream usage)
1. **Config Parse** — `config.toml` is parsed when `omega start` runs. Invalid TOML causes startup error.
2. **Directory Existence** — `~/.omega` must exist; created in Phase 2.
3. **Database Initialize** — On first run, SQLite schema is created in `memory.db`.
4. **Bot Validation** — Telegram bot token is validated when channel tries to connect.

### Why Validation is Minimal in Wizard
- **Fail-Fast Later:** Better to create a config and fail on startup with a clear error than to over-validate during setup.
- **User Flexibility:** Non-critical values (like user_id) can be invalid; user can edit config.toml manually.
- **Token Secrets:** Bot token format can't be validated without calling Telegram API, which we avoid during setup.

---

## LaunchAgent Setup

**Note:** Init wizard does NOT set up LaunchAgent. That's handled separately by `omega service` command.

**Reason:** Wizard is for initial config. Service registration is a separate step that:
- Requires additional permissions
- Is optional (Omega can run manually via `omega start`)
- Is platform-specific (macOS only; Linux uses systemd)

**Related File:** `~/Library/LaunchAgents/com.omega-cortex.omega.plist` (created by service command, not init wizard)

---

## Directory Creation Details

### Omega Data Directory: `~/.omega`

**Created in Phase 2** with `std::fs::create_dir_all(&data_dir)?`

**Structure After Setup:**
```
~/.omega/
  ├── memory.db          # Created on first `omega start`
  ├── omega.log          # Created when logging starts
  ├── SYSTEM_PROMPT.md   # Optional: externalized AI prompts (read at startup)
  ├── WELCOME.toml       # Optional: welcome messages per language (read at startup)
  └── HEARTBEAT.md       # Optional: heartbeat checklist (read by heartbeat loop)
```

**Permissions:** Default user permissions (typically `700` for home subdirectories)

**Cleanup:** If user needs to reset, they can:
- Delete entire `~/.omega` directory
- Re-run `omega init` to regenerate config
- Re-run `omega start` to recreate database

---

## Error Handling Philosophy

The `init.rs` module follows these error handling principles:

1. **I/O Errors are Fatal:** File operations, directory creation, stdin reading use `?` to propagate errors. These abort the wizard.

2. **Missing Claude CLI is Non-Fatal:** The wizard exits gracefully with helpful instructions, not an error. User can fix and retry.

3. **Missing Config File is NOT Fatal:** If `config.toml` already exists, wizard skips generation. No error; just a message.

4. **Invalid User Input is Permissive:** Non-numeric user IDs don't error; they're silently treated as `None`. User can edit config later.

5. **No Panics:** Zero `unwrap()` calls. All fallible operations use `.map()`, `.ok()`, or `?`.

---

## User Experience Considerations

### Time Budget
- **Target:** 2-minute setup
- **Actual:** 1-2 minutes if:
  - Claude CLI already installed
  - User has Telegram token ready
  - User copies/pastes token and user ID

### Accessibility
- **Verbose Prompts:** Each section is labeled with descriptive text
- **Skip Paths:** Optional steps can be skipped (Telegram setup)
- **Clear Instructions:** Links to @BotFather and @userinfobot provided inline
- **Confirmation Messages:** Success and skip messages clearly state what happened

### Failure Modes
- **Claude CLI Missing:** Friendly message with installation command
- **I/O Errors:** Anyhow error message bubbles up; user sees Rust error but can retry
- **Invalid Config Path:** Assumption that CWD is project root; no path validation

---

## Related Components

**Called by:** `src/main.rs` in the `init` command handler

**Reads from:** stdin (user input)

**Writes to:**
- Filesystem: `~/.omega/` directory, `config.toml` file
- stdout: All prompts and messages

**Calls:**
- `std::process::Command` (for `claude --version` check)
- `std::fs` (directory and file operations)
- `std::io` (stdin/stdout)

**Used by:** `omega init` CLI command only; not called by other modules
