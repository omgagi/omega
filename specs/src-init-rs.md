# src/init.rs — Init Wizard Specification

## Path
`src/init.rs`

## Purpose
Interactive setup wizard for new Omega users. Provides a guided onboarding experience using the `cliclack` crate for polished terminal UX. The wizard creates the data directory structure, validates Claude CLI availability, collects Telegram credentials, offers WhatsApp pairing, runs Google Workspace setup via the `gog` CLI, generates the configuration file, and provides next steps. This is the entry point for the `omega init` command.

## Module Overview
The `init.rs` module contains:
- `run()` — Main wizard orchestration function
- `generate_config(bot_token, user_id, whatsapp_enabled, google_email) -> String` — Public pure function that builds `config.toml` content (extracted for testability)
- `run_anthropic_auth()` — Private function that handles Anthropic authentication (setup-token flow)
- `run_whatsapp_setup()` — Private function that handles WhatsApp QR-code pairing
- `run_google_setup()` — Private function that handles Google Workspace OAuth setup via `gog` CLI
- Uses `omega_core::shellexpand()` for home directory expansion (imported, not local)
- Uses `omega_channels::whatsapp` for WhatsApp pairing flow

### UX Layer: cliclack
All user interaction uses the `cliclack` crate instead of raw `println!`/stdin. The following cliclack primitives are used throughout the wizard:

| Primitive | Usage |
|-----------|-------|
| `cliclack::intro(msg)` | Opens the wizard session banner |
| `cliclack::outro(msg)` | Closes the wizard session on success |
| `cliclack::outro_cancel(msg)` | Closes the wizard session on abort |
| `cliclack::input(label)` | Prompted text input with `.placeholder()`, `.required()`, `.default_input()`, `.validate()` |
| `cliclack::confirm(label)` | Yes/No prompt with `.initial_value()` |
| `cliclack::spinner()` | Animated spinner for async/long operations, stopped with `.stop()` or `.error()` |
| `cliclack::note(title, body)` | Boxed informational block |
| `cliclack::log::success(msg)` | Green checkmark log line |
| `cliclack::log::info(msg)` | Info log line |
| `cliclack::log::warning(msg)` | Warning log line |
| `cliclack::log::error(msg)` | Error log line |
| `cliclack::log::step(msg)` | Step progress log line |

---

## Init Wizard Flow

### Phase 1: ASCII Logo and Welcome Banner (unchanged)
**Action:** Display the OMEGA ASCII art logo followed by cliclack intro

**Logo Constant:**
```
const LOGO: &str = r#"
               ██████╗ ███╗   ███╗███████╗ ██████╗  █████╗
              ██╔═══██╗████╗ ████║██╔════╝██╔════╝ ██╔══██╗
              ██║   ██║██╔████╔██║█████╗  ██║  ███╗███████║
              ██║   ██║██║╚██╔╝██║██╔══╝  ██║   ██║██╔══██║
              ╚██████╔╝██║ ╚═╝ ██║███████╗╚██████╔╝██║  ██║
               ╚═════╝ ╚═╝     ╚═╝╚══════╝ ╚═════╝ ╚═╝  ╚═╝
"#;
```

**Logic:**
1. Print `LOGO` via `println!` (raw output, before cliclack takes over)
2. Call `cliclack::intro("omega init")?` to open the styled session

**Purpose:** Creates strong visual branding and immediately signals to the user that they are in an interactive setup experience. The ASCII logo is printed with raw `println!` because it precedes the cliclack session boundary.

**Duration:** Instant

---

### Phase 2: Data Directory Setup
**Action:** Create `~/.omega` if it doesn't exist

**Logic Flow:**
1. Expand `~/` to actual home directory path using `shellexpand("~/.omega")`
2. Check if directory exists with `Path::new(&data_dir).exists()`
3. If missing: Create recursively with `std::fs::create_dir_all(&data_dir)?`; log via `cliclack::log::success(format!("{data_dir} — created"))`
4. If exists: Log via `cliclack::log::success(format!("{data_dir} — exists"))`

**Outputs:**
- Created: `~/.omega — created` (green checkmark)
- Exists: `~/.omega — exists` (green checkmark)

**Error Handling:** Propagate `io::Error` via `?` operator to caller

**Purpose:** Ensures persistent storage location exists before later configuration steps. Directory is required for:
- SQLite database (`memory.db`)
- Log files (`omega.log`)
- Skills directory
- WhatsApp session data
- Service files

---

### Phase 3: Claude CLI Validation
**Action:** Verify `claude` CLI is installed and accessible in PATH

**Logic Flow:**
1. Create a `cliclack::spinner()` and start it with `"Checking claude CLI..."`
2. Execute `claude --version` as subprocess, capture output
3. Map exit status to boolean with `.map(|o| o.status.success()).unwrap_or(false)`
4. If found: Stop spinner with `"claude CLI — found"`
5. If NOT found:
   - Stop spinner with error: `"claude CLI — NOT FOUND"`
   - Display installation instructions via `cliclack::note("Install claude CLI", ...)`
   - Close session with `cliclack::outro_cancel("Setup aborted")?`
   - Return early with `Ok(())` (graceful, non-error exit)

**Outputs (Positive Path):**
```
◇ claude CLI — found
```

**Outputs (Negative Path):**
```
✖ claude CLI — NOT FOUND
┃ Install claude CLI
┃ npm install -g @anthropic-ai/claude-code
┃ Then run 'omega init' again.
◇ Setup aborted
```

**Error Handling:** Non-error early return. The wizard exits gracefully without creating config if Claude CLI is missing. This prevents creating a broken configuration.

**Critical Detail:** Uses `.unwrap_or(false)` to gracefully handle execution failures (e.g., `claude` not in PATH), converting them to `false` instead of panicking.

**Purpose:** Guards against misconfiguration. Users cannot proceed without Claude CLI since it is the default provider.

---

### Phase 3.5: Anthropic Authentication
**Action:** Offer user a choice between "Already authenticated" and pasting a setup-token.

**Logic Flow:**
1. Call `run_anthropic_auth()` function
2. Display `cliclack::select("Anthropic auth method")` with two options:
   - `"Already authenticated (Recommended)"` — Claude CLI is already logged in
   - `"Paste setup-token"` — Run `claude setup-token` elsewhere, then paste here
3. If "Already authenticated": log success via `cliclack::log::success("Anthropic authentication — already configured")`
4. If "Paste setup-token":
   - Display `cliclack::note("Anthropic setup-token", ...)` with instructions
   - Prompt for token via `cliclack::input("Paste Anthropic setup-token")` with validation (non-empty)
   - Start spinner: `"Applying setup-token..."`
   - Execute `claude setup-token <token>` as subprocess
   - On success: stop spinner with `"Anthropic authentication — configured"`
   - On failure: stop spinner with error, warn user to authenticate later

**Error Handling:** Setup-token failures are non-fatal. The wizard continues regardless. User is told they can authenticate later.

**Purpose:** Allows headless/remote setup of the Claude Code CLI by transferring authentication via a setup-token generated on an already-authenticated machine.

---

### Phase 4: Telegram Bot Token Collection
**Action:** Prompt user for Telegram bot token via `cliclack::input`

**Logic Flow:**
1. Call `cliclack::input("Telegram bot token")` with:
   - `.placeholder("Paste token from @BotFather (or Enter to skip)")`
   - `.required(false)`
   - `.default_input("")`
   - `.interact()?`
2. Receive string (empty string if user just presses Enter)
3. If empty: Log via `cliclack::log::info("Skipping Telegram — you can add it later in config.toml")`

**Validation:** No format validation of token at this stage. Invalid tokens are caught later when the bot attempts to connect.

**Purpose:** Collects the core credential needed for Telegram channel integration. Optional because:
- User may want to test Omega locally first
- User may prefer a different messaging platform
- Token can be added to `config.toml` manually later

---

### Phase 5: Telegram User ID Collection (Conditional)
**Condition:** Only asked if `bot_token` is NOT empty

**Action:** Prompt user for Telegram user ID via `cliclack::input`

**Logic Flow:**
1. Call `cliclack::input("Your Telegram user ID")` with:
   - `.placeholder("Send /start to @userinfobot (blank = allow all)")`
   - `.required(false)`
   - `.default_input("")`
   - `.interact()?`
2. Attempt to parse as `i64` with `.parse::<i64>().ok()`
3. If parse succeeds: Store `Some(user_id)`
4. If parse fails or blank: Store `None`

**Parse Result Options:**
- `"123456789"` -> `Some(123456789)`
- `""` (empty) -> `None`
- `"invalid"` (non-numeric) -> `None` (no error shown to user)
- `"999999999999999999"` (overflow) -> `None`

**Purpose:** Enables optional auth filtering. User can:
- Whitelist themselves only: `allowed_users = [123456789]`
- Allow all users who know the token: `allowed_users = []`

---

### Phase 6: WhatsApp Setup
**Action:** Delegate to `run_whatsapp_setup()` private function

**Returns:** `anyhow::Result<bool>` -- `true` if WhatsApp was successfully paired

See [WhatsApp Setup Flow](#whatsapp-setup-flow-run_whatsapp_setup) below for full specification.

---

### Phase 7: Google Workspace Setup
**Action:** Delegate to `run_google_setup()` private function

**Returns:** `anyhow::Result<Option<String>>` -- `Some(email)` if Google was successfully connected

See [Google Workspace Setup Flow](#google-workspace-setup-flow-run_google_setup) below for full specification.

---

### Phase 8: Config File Generation
**Action:** Create or skip `config.toml` based on existing file

**Location:** Current working directory, file named `config.toml`

**Logic Flow:**
1. Check if `config.toml` already exists
2. If exists: Warn via `cliclack::log::warning(...)` and skip generation
3. If missing: Call `generate_config(bot_token, user_id, whatsapp_enabled, google_email)` and write to file

**Skip Output (if exists):**
```
▲ config.toml already exists — skipping.
  Delete it and run 'omega init' again to regenerate.
```

**Success Output:**
```
◇ Generated config.toml
```

**Error Handling:** File write errors propagated via `?` operator. File write failure aborts wizard.

**Important:** Config file is written to CWD, not to `~/.omega`. User should run `omega init` from project root.

---

### Phase 9: Service Installation Offer
**Action:** Prompt user to install Omega as a system service

**Logic Flow:**
1. Prompt with `cliclack::confirm("Install Omega as a system service?")` (initial value: `true`)
2. If user accepts: Call `service::install(config_path)`
   - On success: Record `service_installed = true`
   - On failure: Log warning via `cliclack::log::warning`, suggest `omega service install` later, continue wizard
3. If user declines: Skip (service can be installed later)

**Purpose:** Offers convenient auto-start setup as part of the init flow. Non-fatal — failures don't block the wizard.

**Related:** See `src/service.rs` for full service install specification.

---

### Phase 10: Summary and Next Steps
**Action:** Display next steps via `cliclack::note` and close session with `cliclack::outro`

**Logic Flow:**
1. Build step list starting with base steps:
   ```
   1. Review config.toml
   2. Run: omega start
   3. Send a message to your bot
   ```
2. If `whatsapp_enabled` is `true`, append: `4. WhatsApp is linked and ready!`
3. If `google_email` is `Some(...)`, append: `★ Google Workspace is connected!`
4. If `service_installed` is `true`, append: `★ System service installed — Omega starts on login!`
5. If service was declined, append: `Tip: Run 'omega service install' to auto-start on login`
6. Display via `cliclack::note("Next steps", &steps)?`
7. Close session with `cliclack::outro("Setup complete — enjoy Omega!")?`

**Purpose:**
- Confirms successful wizard completion
- Provides explicit next steps to reduce user confusion
- Reflects which optional integrations and service were configured

---

## WhatsApp Setup Flow (`run_whatsapp_setup`)

### Signature
```rust
fn run_whatsapp_setup() -> anyhow::Result<bool>
```

### Logic Flow
1. Prompt user with `cliclack::confirm("Connect WhatsApp?")` (initial value: `false`)
2. If user declines: return `Ok(false)`
3. Log step and instructions via `cliclack::log::step` and `cliclack::log::info`
4. Spawn a short-lived `tokio::runtime::Runtime` for the async pairing flow
5. Inside the async block:
   - Call `whatsapp::start_pairing("~/.omega").await?` to get QR and done channels
   - Wait up to 30 seconds for the first QR code on `qr_rx`
   - Render QR code via `whatsapp::generate_qr_terminal(&qr_data)?`
   - Display QR code inside `cliclack::note("Scan this QR code with WhatsApp", &qr_text)?`
   - Start a spinner: `"Waiting for scan..."`
   - Wait up to 60 seconds for pairing confirmation on `done_rx`
   - If paired: stop spinner with `"WhatsApp linked successfully"`
   - If not paired: stop spinner with error `"Pairing did not complete"`
6. On success (`Ok(true)`): return `Ok(true)`
7. On failure (`Ok(false)`): warn user, return `Ok(false)`
8. On error (`Err(e)`): log error, return `Ok(false)` (non-fatal)

### Error Handling
All errors are caught and converted to `Ok(false)` with a warning message. WhatsApp setup never fails the wizard.

---

## Google Workspace Setup Flow (`run_google_setup`)

### Signature
```rust
fn run_google_setup() -> anyhow::Result<Option<String>>
```

### Logic Flow

**Step 1: Check `gog` CLI availability**
1. Execute `gog --version` as subprocess
2. Map exit status to boolean with `.map(|o| o.status.success()).unwrap_or(false)`
3. If NOT found: return `Ok(None)` silently (no prompt, no message)

**Step 2: Offer setup**
1. Prompt with `cliclack::confirm("Set up Google Workspace? (Gmail, Calendar, Drive)")` (initial value: `false`)
2. If user declines: return `Ok(None)`

**Step 3: Display Google Cloud Console instructions**
1. Call `cliclack::note("Google Workspace Setup", ...)` with a 6-step guide:
   - Go to console.cloud.google.com
   - Create a project (or use existing)
   - Enable: Gmail API, Calendar API, Drive API
   - Go to Credentials -> Create OAuth Client ID -> Desktop app
   - Download the JSON file
   - Go to OAuth consent screen → Audience → Publish app

**Step 4: Collect credentials file path**
1. Call `cliclack::input("Path to client_secret.json")` with:
   - `.placeholder("~/Downloads/client_secret_xxxxx.json")`
   - `.validate(...)` closure that:
     - Rejects empty input with `"Path is required"`
     - Expands path via `shellexpand(input)`
     - Checks `Path::new(&expanded).exists()` -- rejects with `"File not found"` if missing
   - `.interact()?`
2. Expand the path via `shellexpand(&cred_path)`

**Step 5: Register credentials**
1. Start spinner: `"Running: gog auth credentials ..."`
2. Execute `gog auth credentials <expanded_path>` as subprocess
3. On success: stop spinner with `"Credentials registered"`
4. On failure (non-zero exit): stop spinner with error, log warning, return `Ok(None)`
5. On execution error: stop spinner with error, log warning, return `Ok(None)`

**Step 6: Collect Gmail address**
1. Call `cliclack::input("Your Gmail address")` with:
   - `.placeholder("you@gmail.com")`
   - `.validate(...)` closure that rejects empty input or input without `@` with `"Please enter a valid email address"`
   - `.interact()?`

**Step 7: Display OAuth Tips**
1. Before starting the OAuth flow, display `cliclack::note("OAuth Tips", ...)` with troubleshooting guidance:
   - Open the browser link that appears
   - Use an incognito/private window if you have trouble
   - Ensure your app is published in OAuth consent screen
   - Add yourself as a test user if not using a published app

**Step 8: OAuth authorization**
1. Start spinner: `"Waiting for OAuth approval in browser..."`
2. Execute `gog auth add <email> --services gmail,calendar,drive,contacts,docs,sheets` as subprocess
3. On success: stop spinner with `"OAuth approved"`
4. On failure (non-zero exit): stop spinner with error, log warning, display incognito retry suggestion, return `Ok(None)`
5. On execution error: stop spinner with error, log warning, display incognito retry suggestion, return `Ok(None)`

**Step 9: Verify with `gog auth list`**
1. Execute `gog auth list` as subprocess
2. Check if stdout contains the email address
3. If verified: log success `"Google Workspace connected!"`, return `Ok(Some(email))`
4. If not verified: log warning `"Could not verify Google auth — check manually with 'gog auth list'."`, return `Ok(Some(email))` (auth might still have worked)

### Error Handling
- Missing `gog` CLI: silently skipped, user is never prompted
- All `gog` subprocess failures produce warnings and return `Ok(None)` -- never fatal to the wizard
- Failed OAuth now shows incognito retry suggestion to help users troubleshoot browser-related issues
- Verification failure is non-fatal; returns the email optimistically

---

## Config Generation (`generate_config`)

### Signature
```rust
pub fn generate_config(
    bot_token: &str,
    user_id: Option<i64>,
    whatsapp_enabled: bool,
    google_email: Option<&str>,
) -> String
```

### Purpose
Public pure function that builds the `config.toml` content string. Extracted from the wizard flow for testability -- no I/O, no side effects.

### Parameter Mapping

| Parameter | Config Effect |
|-----------|--------------|
| `bot_token` (empty) | `[channel.telegram] enabled = false`, `bot_token = ""` |
| `bot_token` (non-empty) | `[channel.telegram] enabled = true`, `bot_token = "<value>"` |
| `user_id = Some(id)` | `allowed_users = [<id>]` |
| `user_id = None` | `allowed_users = []` |
| `whatsapp_enabled = true` | `[channel.whatsapp] enabled = true` |
| `whatsapp_enabled = false` | `[channel.whatsapp] enabled = false` |
| `google_email = Some(email)` | Appends `[google] account = "<email>"` section |
| `google_email = None` | No `[google]` section in output |

### Configuration Template Structure

```toml
[omega]
name = "Ω OMEGA"
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

[channel.whatsapp]
enabled = {wa_enabled}
allowed_users = []

[memory]
backend = "sqlite"
db_path = "~/.omega/memory.db"
max_context_messages = 50

# Appended only when google_email is Some:
[google]
account = "{email}"
```

### Default Values Applied
- Log level: `info`
- Provider default: `claude-code`
- Max turns per provider: `10`
- Allowed tools: `["Bash", "Read", "Write", "Edit"]` (pre-selected safe tools)
- Memory backend: `sqlite`
- Max context messages: `50`
- Auth: Enabled
- WhatsApp allowed_users: always `[]` (configured separately)

---

## Unit Tests

### Test Suite: `tests` (5 tests)

All tests exercise the `generate_config()` pure function. No I/O mocking required.

| Test | Parameters | Assertions |
|------|-----------|------------|
| `test_generate_config_full` | `("123:ABC", Some(42), true, Some("me@gmail.com"))` | Token present, user ID in array, telegram enabled, whatsapp enabled, google section present with email |
| `test_generate_config_minimal` | `("", None, false, None)` | Empty token, empty allowed_users, telegram disabled, whatsapp disabled, no google section |
| `test_generate_config_telegram_only` | `("tok:EN", Some(999), false, None)` | Token present, user ID in array, telegram enabled, whatsapp disabled, no google section |
| `test_generate_config_google_only` | `("", None, false, Some("test@example.com"))` | Telegram disabled, google section present with email |
| `test_generate_config_whatsapp_only` | `("", None, true, None)` | Whatsapp enabled, telegram disabled, no google section |

---

## Configuration File Generation Details

### Template Design Rationale

**`[omega]` Section:**
- `name` -- Used for display/logging. Default is "Omega".
- `data_dir` -- Persistent storage location. Pre-configured to `~/.omega`.
- `log_level` -- Defaults to `info`. User can change to `debug` for troubleshooting.

**`[auth]` Section:**
- `enabled = true` -- Enforces auth checks. Per CLAUDE.md security constraints, auth is always on.

**`[provider]` Section:**
- `default` -- Specifies which provider backend to use. Hardcoded to `claude-code` (the only fully-implemented provider).

**`[provider.claude-code]` Section:**
- `enabled` -- Must be true since it is the default.
- `max_turns` -- Safety limit on Claude Code provider context window. Set to 10 to prevent runaway conversations.
- `allowed_tools` -- Whitelist of tools Claude Code can use. Restricted to safe subset:
  - `Bash` -- Execute commands
  - `Read` -- Read files
  - `Write` -- Write files
  - `Edit` -- Edit files
  - **Excluded:** `Skill` (plugin system), others for safety

**`[channel.telegram]` Section:**
- `enabled` -- Boolean, set based on whether user provided token
- `bot_token` -- Sensitive! Should be environment variable in production, but template allows inline for simplicity
- `allowed_users` -- List of Telegram user IDs allowed to send messages. Empty allows all.

**`[channel.whatsapp]` Section:**
- `enabled` -- Boolean, set based on whether WhatsApp pairing succeeded
- `allowed_users` -- Always `[]` (empty); configured separately by user

**`[google]` Section (optional):**
- Only included when Google Workspace setup succeeded
- `account` -- The Gmail address used for OAuth authorization

**`[memory]` Section:**
- `backend` -- Hardcoded to `sqlite` (only option currently)
- `db_path` -- Where SQLite database is stored. Typically `~/.omega/memory.db`.
- `max_context_messages` -- How many historical messages to include in prompt context. Set to 50 as a balance between context and token cost.

---

## Validation Steps

### Pre-Setup Validation
1. **Claude CLI Check** -- Only critical validation. Wizard exits gracefully if missing.

### In-Wizard Validation
1. **Google credentials path** -- Must be non-empty and point to an existing file (validated by `cliclack::input.validate()`)
2. **Gmail address** -- Must be non-empty and contain `@` (validated by `cliclack::input.validate()`)

### Post-Setup Validation (Not in `init.rs`, but in downstream usage)
1. **Config Parse** -- `config.toml` is parsed when `omega start` runs. Invalid TOML causes startup error.
2. **Directory Existence** -- `~/.omega` must exist; created in Phase 2.
3. **Database Initialize** -- On first run, SQLite schema is created in `memory.db`.
4. **Bot Validation** -- Telegram bot token is validated when channel tries to connect.

### Why Validation is Minimal in Wizard
- **Fail-Fast Later:** Better to create a config and fail on startup with a clear error than to over-validate during setup.
- **User Flexibility:** Non-critical values (like user_id) can be invalid; user can edit config.toml manually.
- **Token Secrets:** Bot token format cannot be validated without calling Telegram API, which we avoid during setup.

---

## LaunchAgent Setup

**Note:** Init wizard does NOT set up LaunchAgent. That is handled separately by the `omega service` command.

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
  ├── skills/            # Optional: user skill definitions
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

1. **I/O Errors are Fatal:** File operations, directory creation, cliclack interactions use `?` to propagate errors. These abort the wizard.

2. **Missing Claude CLI is Non-Fatal:** The wizard exits gracefully with helpful instructions via `cliclack::outro_cancel`, not an error. User can fix and retry.

3. **Existing Config File is NOT Fatal:** If `config.toml` already exists, wizard skips generation with a `cliclack::log::warning`. No error; just a message.

4. **Invalid User Input is Permissive:** Non-numeric user IDs don't error; they are silently treated as `None`. User can edit config later.

5. **WhatsApp Failures are Non-Fatal:** All errors are caught and converted to `Ok(false)` with a warning.

6. **Google Setup Failures are Non-Fatal:** Missing `gog` CLI is silently skipped. All subprocess failures produce warnings and return `Ok(None)`.

7. **No Panics:** Zero `unwrap()` calls in the happy path. The only `unwrap_or(false)` calls are on subprocess exit status checks, converting execution failures to `false`.

---

## User Experience Considerations

### Time Budget
- **Target:** 2-3 minutes for full setup (Telegram + WhatsApp + Google)
- **Actual:** 1-2 minutes if:
  - Claude CLI already installed
  - User has Telegram token ready
  - WhatsApp and Google are skipped

### cliclack UX Benefits
- **Styled prompts:** Consistent visual language with checkmarks, spinners, and boxed notes
- **Placeholder text:** Guides user on expected input format without cluttering the prompt
- **Validation feedback:** Inline error messages for file path and email validation
- **Spinners:** Visual feedback during subprocess execution (Claude CLI check, gog commands)
- **Session boundaries:** `intro`/`outro` clearly delineate the wizard session

### Skip Paths
- **Telegram:** Leave bot token blank to skip
- **User ID:** Leave blank or enter non-numeric to allow all users
- **WhatsApp:** Decline the confirm prompt
- **Google Workspace:** Automatically skipped if `gog` CLI is not installed; otherwise decline the confirm prompt

### Failure Modes
- **Claude CLI Missing:** Styled error with installation command inside `cliclack::note`, session closed with `cliclack::outro_cancel`
- **WhatsApp Pairing Timeout:** Spinner stops with error, warning logged, wizard continues
- **Google `gog` Failures:** Spinner stops with error, warning logged, wizard continues
- **I/O Errors:** Anyhow error message bubbles up; user sees Rust error but can retry

---

## Related Components

**Called by:** `src/main.rs` in the `init` command handler

**Reads from:** stdin (via cliclack interactive prompts)

**Writes to:**
- Filesystem: `~/.omega/` directory, `config.toml` file
- stdout: All prompts and messages (via cliclack)

**Dependencies:**
- `cliclack` -- Terminal UX primitives (intro, outro, input, confirm, spinner, note, log)
- `omega_core::shellexpand` -- Home directory expansion
- `omega_channels::whatsapp` -- WhatsApp QR pairing flow
- `std::process::Command` -- Subprocess execution (`claude --version`, `gog` commands)
- `std::fs` -- Directory and file operations
- `tokio::runtime::Runtime` -- Short-lived async runtime for WhatsApp pairing
- `anyhow` -- Error handling

**Used by:** `omega init` CLI command only; not called by other modules
