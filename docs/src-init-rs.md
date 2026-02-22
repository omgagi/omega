# src/init.rs — Init Wizard Documentation

## Overview

The init wizard is Omega's **first impression and first-time user experience**. It's a 2-minute interactive setup that transforms Omega from an uninitialized Rust project into a working personal AI agent connected to your messaging platforms and services.

The wizard uses the **`cliclack`** crate for a polished, styled CLI experience with `◆ ◇ │` visual markers, spinners, confirmation toggles, and structured note blocks — replacing the plain `println!` prompts of earlier versions.

The init wizard solves a critical problem: **new users need a frictionless path from "I cloned this repo" to "my bot works."** Without it, new users would face:
- Manual directory creation
- Manual config file writing (or copying example)
- Uncertainty about required credentials
- Risk of misconfiguration

The wizard eliminates these friction points through guided, interactive setup.

---

## What is `omega init`?

`omega init` is the **onboarding command** for Omega. It's the first thing a new user runs after cloning the repository. It supports two modes: **interactive** (guided wizard) and **non-interactive** (programmatic deployment via CLI args or env vars).

### Command Usage

**Interactive mode** (default):
```bash
omega init
```

**Non-interactive mode** (when `--telegram-token` or `--allowed-users` is provided):
```bash
omega init --telegram-token "123:ABC" --allowed-users "842277204,123456"
```

### Execution Context
- **Requires:** Rust environment (user has already built/run Omega)
- **Runs:** Synchronously in the terminal where user types
- **Duration:** 1-3 minutes including user input time (interactive); seconds (non-interactive)
- **Output:** cliclack-styled prompts, spinners, confirmations, and next-step instructions (interactive); tracing log messages (non-interactive)
- **Side Effects:** Creates `~/.omega/`, generates `config.toml`, validates Claude CLI, optionally pairs WhatsApp, optionally connects Google Workspace

### Success Criteria
User can run `omega start` immediately after and have a working bot.

---

## Non-Interactive Mode (Programmatic Deployment)

When `--telegram-token` or `--allowed-users` is provided via CLI arguments or `OMEGA_` environment variables, `omega init` skips the interactive wizard entirely and performs a programmatic deployment. This is useful for:

- Automated server provisioning (Docker, CI/CD)
- Scripted deployments
- Headless machines without TTY

### Usage Examples

**Minimal deployment with CLI args:**
```bash
omega init --telegram-token "123:ABC" --allowed-users "842277204,123456"
```

**Using environment variables:**
```bash
OMEGA_TELEGRAM_TOKEN="123:ABC" OMEGA_ALLOWED_USERS="842277204" omega init
```

**Full deployment with all options:**
```bash
omega init --telegram-token "123:ABC" --allowed-users "842277204" \
  --claude-setup-token "..." --whisper-key "sk-..." \
  --google-credentials ~/client_secret.json --google-email user@gmail.com
```

### Available Arguments

| Argument | Env Var | Default | Purpose |
|----------|---------|---------|---------|
| `--telegram-token` | `OMEGA_TELEGRAM_TOKEN` | — | Telegram bot token from @BotFather |
| `--allowed-users` | `OMEGA_ALLOWED_USERS` | — | Comma-separated Telegram user IDs |
| `--claude-setup-token` | `OMEGA_CLAUDE_SETUP_TOKEN` | — | Anthropic setup token for headless Claude CLI auth |
| `--whisper-key` | `OMEGA_WHISPER_KEY` | — | OpenAI API key for Whisper voice transcription |
| `--google-credentials` | `OMEGA_GOOGLE_CREDENTIALS` | — | Path to Google OAuth `client_secret.json` |
| `--google-email` | `OMEGA_GOOGLE_EMAIL` | — | Gmail address for Google Workspace |

### Non-Interactive Flow Steps

1. **Parse arguments** -- Validate `--allowed-users` (comma-separated integers)
2. **Create data directory** -- `~/.omega/` created if missing
3. **Validate Claude CLI** -- Same check as interactive mode (`claude --version`)
4. **Apply setup token** -- If `--claude-setup-token` provided, runs `claude setup-token <token>`
5. **Register Google credentials** -- If `--google-credentials` provided, runs `gog auth credentials <path>`
6. **Generate config** -- Writes `config.toml` from provided arguments (skips if file exists)
7. **Install service** -- Calls `service::install_quiet()` for non-interactive service installation

### Error Handling
- Invalid `--allowed-users` (non-numeric values) causes immediate failure with a descriptive error
- Missing Claude CLI is fatal (same as interactive mode)
- Google credential failures are non-fatal (warning logged, deployment continues)

---

## The Onboarding Experience: Step-by-Step

### Step 1: Welcome Banner (Instant)

**What the User Sees:**
```
              ██████╗ ███╗   ███╗███████╗ ██████╗  █████╗        █████╗
             ██╔═══██╗████╗ ████║██╔════╝██╔════╝ ██╔══██╗      ██╔══██╗
             ██║   ██║██╔████╔██║█████╗  ██║  ███╗███████║      ██║  ██║
             ██║   ██║██║╚██╔╝██║██╔══╝  ██║   ██║██╔══██║      ╚██╗██╔╝
             ╚██████╔╝██║ ╚═╝ ██║███████╗╚██████╔╝██║  ██║    ████╔╝╚████╗
              ╚═════╝ ╚═╝     ╚═╝╚══════╝ ╚═════╝ ╚═╝  ╚═╝    ╚═══╝  ╚═══╝

┌  omega init
│
```

**What's Happening:**
The wizard prints the ASCII OMEGA banner, then calls `cliclack::intro("omega init")` to begin the styled wizard session. The `┌` marker signals the start of the interactive flow.

**User Action:** None. Just read the banner.

**Time:** < 1 second

---

### Step 2: Create Data Directory (< 1 second)

**What the User Sees (Success):**
```
◇  ~/.omega — created
```

**What the User Sees (If Already Exists):**
```
◇  ~/.omega — exists
```

**What's Happening:**
The wizard creates `~/.omega`, a hidden directory in the user's home directory where Omega will store:
- SQLite database (`data/memory.db`) — conversation history and memory
- Log files (`logs/omega.log`) — runtime logs
- Skills (`skills/*.md`) — loaded at startup
- WhatsApp session data (if paired)
- System prompt (`prompts/SYSTEM_PROMPT.md`) — optional custom prompt
- Welcome messages (`prompts/WELCOME.toml`) — optional per-language greetings
- Heartbeat checklist (`prompts/HEARTBEAT.md`) — optional self-check items

**Why It Matters:**
Without this directory, Omega can't persist data between sessions. Creating it upfront ensures the user won't see mysterious "directory not found" errors later.

**User Action:** None. The wizard creates this automatically.

**Time:** < 1 second

---

### Step 3: Validate Claude CLI (1-3 seconds)

**What the User Sees (While Checking):**
```
◒  Checking claude CLI...
```

**What the User Sees (Success):**
```
◇  claude CLI — found
```

**What the User Sees (Failure):**
```
▲  claude CLI — NOT FOUND
│
│  Install claude CLI
│
│  npm install -g @anthropic-ai/claude-code
│
│  Then run 'omega init' again.
│
└  Setup aborted
```

**What's Happening:**
A `cliclack::spinner()` animates while the wizard runs `claude --version` to verify that the Claude Code CLI is installed and accessible. Claude Code is Omega's default AI backend, so it's **mandatory** for Omega to work.

**Why It Matters:**
If Claude CLI is missing, Omega cannot function. Rather than letting the user discover this later during `omega start`, the wizard fails fast with a `cliclack::note` containing the installation command. The session ends with `cliclack::outro_cancel("Setup aborted")`.

**User Action:**
- If found: Proceed to next step
- If not found: User must install Claude CLI via npm, then re-run `omega init`

**Time:** 1-3 seconds (includes subprocess execution)

**Implementation Detail:**
The wizard uses `.unwrap_or(false)` to gracefully handle execution failures. If the `claude` command can't be found, the check fails safely without panicking, showing the user-friendly error message.

---

### Step 3.5: Anthropic Authentication (< 30 seconds)

**What the User Sees:**
```
◆  Anthropic auth method
│  ● Already authenticated (Recommended) — Claude CLI is already logged in
│  ○ Paste setup-token — Run `claude setup-token` elsewhere, then paste here
```

**What's Happening:**
After verifying the Claude CLI is installed, the wizard asks how the user wants to authenticate with Anthropic. Most users who already have `claude` working will select "Already authenticated".

**If User Selects "Already authenticated":**
```
◇  Anthropic authentication — already configured
```

**If User Selects "Paste setup-token":**
```
│
│  Anthropic setup-token
│
│  Run `claude setup-token` in your terminal.
│  Then paste the generated token below.
│
◆  Paste Anthropic setup-token
│  Paste the token here
│  _
```

After pasting, the wizard runs `claude setup-token <token>`:

**On Success:**
```
◇  Anthropic authentication — configured
```

**On Failure:**
```
▲  setup-token failed: <error>
◇  You can authenticate later with: claude setup-token
```

**Why This Exists:**
When setting up Omega on a new/headless machine, the Claude CLI needs authentication. The `claude setup-token` command on an already-authenticated machine generates a transferable token. This wizard step lets users paste that token to authenticate the CLI without a browser.

**Time:** < 30 seconds (instant if already authenticated)

---

### Step 4: Telegram Bot Setup -- Token Collection (< 1 minute)

**What the User Sees:**
```
◆  Telegram bot token
│  Paste token from @BotFather (or Enter to skip)
│  _
```

The `◆` marker indicates an active input prompt waiting for the user.

**What's Happening:**
The wizard uses `cliclack::input()` with a placeholder hint to prompt the user for a Telegram bot token. This token allows Omega to receive messages from Telegram users and send responses back.

**Where Does the Token Come From?**
New Telegram users who don't have a bot:
1. Open Telegram
2. Search for `@BotFather`
3. Send `/newbot`
4. Follow BotFather's prompts to name the bot
5. BotFather responds with a bot token
6. Copy/paste that token into this prompt

**User Action Options:**

**Option A: User Has Token Ready**
```
◆  Telegram bot token
│  123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11
```
User pastes token, presses Enter. Wizard stores the token and proceeds.

**Option B: User Doesn't Have Token Yet**
```
◆  Telegram bot token
│
```
User just presses Enter (leaves blank). Wizard skips Telegram setup:
```
◇  Skipping Telegram — you can add it later in config.toml
```

**Why Skipping is OK:**
Telegram integration is powerful but not required. Users might want to:
- Test Omega locally first without connecting to Telegram
- Integrate with WhatsApp instead
- Set up Telegram token manually later

The wizard's philosophy: **Don't block the user on optional features.**

**Time:** 30 seconds to 1 minute (includes user time to find/copy token)

---

### Step 5: Telegram User ID -- Optional Allowlist (Optional, < 30 seconds)

**What the User Sees (Only if Token Was Provided):**
```
◆  Your Telegram user ID
│  Send /start to @userinfobot (blank = allow all)
│  _
```

**What's Happening:**
If the user provided a Telegram token, the wizard optionally asks for their Telegram user ID. This enables **auth filtering**: only specified users can send messages to the bot.

**Two Scenarios:**

**Scenario 1: User Provides Their ID**
```
◆  Your Telegram user ID
│  123456789
```
The wizard records this ID. Later, the bot will only respond to messages from this specific Telegram user. This is secure; the bot ignores everyone else.

**Scenario 2: User Leaves Blank**
```
◆  Your Telegram user ID
│
```
The wizard records `None` (no ID). The bot accepts messages from any Telegram user who knows the bot. This is useful for:
- Testing the bot locally without auth restrictions
- Shared bots or group deployments
- Later adding auth via manual config editing

**How to Find User ID:**
1. In Telegram, search for `@userinfobot`
2. Send `/start`
3. Bot responds with your user ID number
4. Copy/paste into this prompt

**Important:** This step is skipped entirely if the user didn't provide a bot token in the previous step. If Telegram is disabled, there's no reason to collect user IDs.

**Time:** Optional; 20-30 seconds if performed

---

### Step 6: WhatsApp Setup (Optional, 1-2 minutes if performed)

**What the User Sees:**
```
◆  Connect WhatsApp?
│  No / Yes
│
```

This is a `cliclack::confirm()` toggle with a default of `No`. The user can toggle between Yes and No using arrow keys or type `y`/`n`.

**If User Selects No:**
The wizard moves on. WhatsApp will be set to `enabled = false` in the config.

**If User Selects Yes:**
```
◇  Starting WhatsApp pairing...
◇  Open WhatsApp on your phone > Linked Devices > Link a Device
│
│  Scan this QR code with WhatsApp
│
│  ████████████████████████████████
│  ██ ▄▄▄▄▄ █ ▄▀ ▀█ ▄▀█ ▄▄▄▄▄ ██
│  ██ █   █ █ █▀ ▀ ▀█▄█ █   █ ██
│  ...
│  ████████████████████████████████
│
◒  Waiting for scan...
```

The wizard:
1. Spins up a temporary tokio runtime for the async pairing flow
2. Calls `whatsapp::start_pairing("~/.omega")` to begin the pairing process
3. Waits up to 30 seconds for a QR code to appear
4. Renders the QR code inside a `cliclack::note` block
5. Shows a spinner while waiting up to 60 seconds for the user to scan
6. Reports success or failure

**On Success:**
```
◇  WhatsApp linked successfully
```

**On Failure or Timeout:**
```
▲  Pairing did not complete
◇  You can try again later with /whatsapp.
```

**Time:** 1-2 minutes if the user pairs; instant if skipped

---

### Step 7: Google Workspace Setup (Optional, 2-3 minutes if performed)

This step **only appears if the `gog` CLI tool is installed** on the system. If `gog` is not found, the wizard silently skips this step -- the user never sees any Google-related prompts.

**Detection Check (Invisible to User):**
The wizard runs `gog --version` behind the scenes. If it fails, this entire step is skipped.

**What the User Sees (if `gog` is installed):**
```
◆  Set up Google Workspace? (Gmail, Calendar, Drive)
│  No / Yes
│
```

This is a `cliclack::confirm()` toggle defaulting to `No`.

**If User Selects No:**
The wizard moves on. No `[google]` section is added to the config.

**If User Selects Yes:**

**Sub-step 7a: Setup Instructions**
```
│
│  Google Workspace Setup
│
│  1. Go to console.cloud.google.com
│  2. Create a project (or use existing)
│  3. Enable: Gmail API, Calendar API, Drive API
│  4. Go to Credentials → Create OAuth Client ID → Desktop app
│  5. Download the JSON file
│  6. Go to OAuth consent screen → Audience → Publish app
│
```

The wizard displays step-by-step Google Cloud Console instructions inside a `cliclack::note` block so the user knows how to obtain the credentials file.

**Sub-step 7b: Credentials File Path**
```
◆  Path to client_secret.json
│  ~/Downloads/client_secret_xxxxx.json
│  _
```

The user enters the path to their downloaded `client_secret.json` file. This input is **validated** -- the wizard checks that the file exists (after shell expansion of `~`) and will show an error if the path is empty or the file is not found.

**Sub-step 7c: Register Credentials**
```
◒  Running: gog auth credentials ...
◇  Credentials registered
```

The wizard runs `gog auth credentials <path>` to register the OAuth client credentials with the `gog` tool.

If this fails, the wizard shows the error and skips the rest of Google setup:
```
▲  gog auth credentials failed: <error details>
◇  Skipping Google Workspace setup.
```

**Sub-step 7d: Gmail Address**
```
◆  Your Gmail address
│  you@gmail.com
│  _
```

The user enters their Gmail address. Validated to be non-empty and contain an `@` sign.

**Sub-step 7e: Incognito Browser Offer**

The wizard automatically detects installed browsers that support incognito/private mode (Google Chrome, Brave, Firefox, Microsoft Edge) by checking `/Applications/*.app`.

If at least one is found:
```
◆  Open OAuth URL in incognito/private window? (recommended)
│  Yes / No
```

If the user selects Yes and multiple browsers are available:
```
◆  Which browser?
│  ● Google Chrome
│  ○ Firefox
```

The wizard creates a temporary shell script at `$TMPDIR/omega_incognito_browser.sh` that opens URLs in the selected browser's private mode (e.g., `open -na 'Google Chrome' --args --incognito "$1"`), then passes it via the `BROWSER` environment variable when invoking `gog auth add`.

If no browsers are detected, this step is silently skipped and the default browser is used.

**Sub-step 7f: OAuth Tips**
```
│
│  OAuth Tips
│
│  A browser will open for Google sign-in.
│  • Click 'Advanced' → 'Go to gog (unsafe)' → Allow
│  • If 'Access blocked: not verified', go to OAuth consent screen →
│    Audience → Publish app (or add yourself as a test user)
│
```

The wizard displays troubleshooting guidance before the OAuth flow begins.

**Sub-step 7g: OAuth Approval**
```
◒  Waiting for OAuth approval in browser...
```

The wizard runs `gog auth add <email> --services gmail,calendar,drive,contacts,docs,sheets` with the `BROWSER` env var set if incognito was selected. This opens the browser for Google OAuth consent. A spinner waits while the user approves in the browser. The temporary incognito script is cleaned up after the command completes.

**On Success:**
```
◇  OAuth approved
```

**On Failure:**
```
▲  gog auth add failed: <error details>
◇  If your browser showed an error, try manually in an incognito window:
    gog auth add <email> --services gmail,calendar,drive,contacts,docs,sheets
```

**Sub-step 7h: Verification**

The wizard runs `gog auth list` and checks if the user's email appears in the output.

**On Success:**
```
◇  Google Workspace connected!
```

**On Ambiguous Result:**
```
◇  Could not verify Google auth — check manually with 'gog auth list'.
```

Even if verification is ambiguous, the wizard still records the email in the config (the auth may have worked even if the list command had issues).

**Time:** 2-3 minutes including browser OAuth flow; instant if skipped or if `gog` is not installed

---

### Step 8: Generate Configuration File (< 1 second)

> **Note:** Sandbox mode selection has been removed. Filesystem protection is now always-on via `omega_sandbox`'s blocklist approach -- no configuration needed.

> **Note:** Step numbering continues from Step 7 (Google Workspace). Steps 8-10 are the final wizard phases.

**What the User Sees (Success):**
```
◇  Generated config.toml
```

**What the User Sees (If Config Already Exists):**
```
▲  config.toml already exists — skipping.
│  Delete it and run 'omega init' again to regenerate.
```

**What's Happening:**
The wizard creates `config.toml`, the main configuration file that Omega reads on startup. The config file is **generated based on all the user's inputs** (token, user ID, WhatsApp pairing, Google email).

**The Generated Config (Full Example)**
```toml
[omega]
name = "OMEGA Ω"
data_dir = "~/.omega"
log_level = "info"

[auth]
enabled = true

[provider]
default = "claude-code"

[provider.claude-code]
enabled = true
max_turns = 10
allowed_tools = []  # empty = full tool access

[channel.telegram]
enabled = true
bot_token = "123456:ABC-DEF1234..."
allowed_users = [123456789]

[channel.whatsapp]
enabled = true
allowed_users = []

[memory]
backend = "sqlite"
db_path = "~/.omega/data/memory.db"
max_context_messages = 50

[google]
account = "you@gmail.com"
```

**What Each Section Means:**

| Section | Purpose |
|---------|---------|
| `[omega]` | Global Omega settings (name, storage path, log level) |
| `[auth]` | Authentication enforcement (always enabled) |
| `[provider]` | Which AI backend to use (claude-code is default) |
| `[provider.claude-code]` | Claude Code specific settings (max turns, allowed tools) |
| `[channel.telegram]` | Telegram integration (token, allowed users) |
| `[channel.whatsapp]` | WhatsApp integration (enabled/disabled, allowed users) |
| `[memory]` | Conversation storage (SQLite database settings) |
| `[google]` | Google Workspace account (only present if configured) |

**Config Generation Logic:**
- `[channel.telegram] enabled` = `true` if a bot token was provided, `false` otherwise
- `[channel.whatsapp] enabled` = `true` if WhatsApp was successfully paired, `false` otherwise
- `[google]` section = only included if Google Workspace was connected (contains the email)
- `allowed_users` for Telegram = contains the user ID if provided, empty array otherwise

**Why Config is Generated:**
Rather than making users manually edit a config template, the wizard generates a working config based on their choices. This eliminates errors like:
- Forgetting to change a placeholder value
- Invalid TOML syntax
- Mismatched credentials and allowed_users

**What Happens if Config Already Exists?**
The wizard skips generation to prevent overwriting a user's customized config. If the user wants a fresh config, they delete the old one and re-run `omega init`.

**Where is config.toml Located?**
Current working directory (typically the project root). The user should run `omega init` from the directory where they cloned the Omega repository.

**Time:** < 1 second (write operation)

---

### Step 10: System Service Installation (Optional, < 10 seconds)

**What the User Sees:**
```
◆  Install Omega as a system service?
│  Yes / No
```

This is a `cliclack::confirm()` toggle defaulting to `Yes`. If the user accepts, the wizard calls `service::install()` to create and activate the service file (LaunchAgent on macOS, systemd on Linux).

**On Success:**
The service module's own cliclack output is displayed (binary path, config path, service file, activation status).

**On Failure:**
```
▲  Service install failed: <error>
◇  You can install later with: omega service install
```

The wizard continues regardless — service installation is never fatal.

**If User Declines:**
No service is installed. The "Next steps" summary will include a tip about `omega service install`.

For full service management details, see the [service documentation](src-service-rs.md).

---

### Step 11: Success Message and Next Steps (Instant)

**What the User Sees:**
```
│
│  Next steps
│
│  1. Review config.toml
│  2. Run: omega start
│  3. Send a message to your bot
│  4. WhatsApp is linked and ready!
│  ★ Google Workspace is connected!
│  ★ System service installed — Omega starts on login!
│
└  Setup complete — enjoy Omega!
```

Lines 4, Google, and service lines only appear if those integrations were set up during the wizard. If the user declined the service, a tip line appears instead:
```
Tip: Run `omega service install` to auto-start on login
```

The `└` marker from `cliclack::outro` signals the end of the wizard session.

**What Should the User Do?**

**Step 1: Review config.toml**
The user should open `config.toml` in a text editor and:
- Verify the bot token and user ID are correct
- Adjust settings like `log_level` (change to `debug` for troubleshooting)
- Review allowed tools (empty array = full tool access by default; add specific tool names to restrict)

This step ensures the user understands what they just configured.

**Step 2: Run `omega start`**
```bash
omega start
```

This starts the Omega daemon. It will:
1. Load `config.toml`
2. Initialize the SQLite database
3. Connect to enabled channels (Telegram, WhatsApp)
4. Start listening for incoming messages
5. Log all activity to `~/.omega/logs/omega.log`

**Step 3: Send a Message to the Bot**
In Telegram or WhatsApp, find the bot and send it a message, e.g.:
```
Hello Omega, what time is it?
```

The bot will:
1. Receive your message
2. Check auth (verify your user ID matches)
3. Delegate to Claude Code CLI
4. Get Claude's reasoning response
5. Send response back to the channel
6. Store the conversation in memory

**Time:** < 1 second (display)

---

## Complete Session Example

Here is a complete example of what a full wizard session looks like with all integrations enabled:

```
              ██████╗ ███╗   ███╗███████╗ ██████╗  █████╗        █████╗
             ██╔═══██╗████╗ ████║██╔════╝██╔════╝ ██╔══██╗      ██╔══██╗
             ██║   ██║██╔████╔██║█████╗  ██║  ███╗███████║      ██║  ██║
             ██║   ██║██║╚██╔╝██║██╔══╝  ██║   ██║██╔══██║      ╚██╗██╔╝
             ╚██████╔╝██║ ╚═╝ ██║███████╗╚██████╔╝██║  ██║    ████╔╝╚████╗
              ╚═════╝ ╚═╝     ╚═╝╚══════╝ ╚═════╝ ╚═╝  ╚═╝    ╚═══╝  ╚═══╝

┌  omega init
│
◇  ~/.omega — created
◇  claude CLI — found
│
◆  Anthropic auth method
│  Already authenticated (Recommended)
│
◇  Anthropic authentication — already configured
│
◆  Telegram bot token
│  Paste token from @BotFather (or Enter to skip)
│  123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11
│
◆  Your Telegram user ID
│  Send /start to @userinfobot (blank = allow all)
│  987654321
│
◆  Connect WhatsApp?
│  Yes
│
◇  Starting WhatsApp pairing...
◇  Open WhatsApp on your phone > Linked Devices > Link a Device
│
│  Scan this QR code with WhatsApp
│  [QR CODE]
│
◇  WhatsApp linked successfully
│
◆  Set up Google Workspace? (Gmail, Calendar, Drive)
│  Yes
│
│  Google Workspace Setup
│  1. Go to console.cloud.google.com
│  2. Create a project (or use existing)
│  3. Enable: Gmail API, Calendar API, Drive API
│  4. Go to Credentials → Create OAuth Client ID → Desktop app
│  5. Download the JSON file
│  6. Go to OAuth consent screen → Audience → Publish app
│
◆  Path to client_secret.json
│  ~/Downloads/client_secret_12345.json
│
◇  Credentials registered
│
◆  Your Gmail address
│  user@gmail.com
│
│  OAuth Tips
│
│  Open the browser link that appears
│  Use an incognito/private window if you have trouble
│  Ensure your app is published in OAuth consent screen
│  Add yourself as a test user if not using a published app
│
◇  OAuth approved
◇  Google Workspace connected!
│
◆  Sandbox mode
│  Sandbox (Recommended)
│
◇  Generated config.toml
│
◆  Install Omega as a system service?
│  Yes
│
┌  omega service install
│  ...
└  Omega will now start automatically on login
│
│  Next steps
│
│  1. Review config.toml
│  2. Run: omega start
│  3. Send a message to your bot
│  4. WhatsApp is linked and ready!
│  ★ Google Workspace is connected!
│  ★ System service installed — Omega starts on login!
│
└  Setup complete — enjoy Omega!
```

---

## Complete First-Time User Journey

Here's what a new user experiences from start to finish:

```
User clones repo
       |
User reads README
       |
User runs: cargo build --release
       |
User runs: omega init
       |
[WIZARD BEGINS — cliclack session]
       |
1. ASCII OMEGA banner displayed
2. ~/.omega directory created (or confirmed)
3. Claude CLI validated via spinner
4. Anthropic authentication (already auth or setup-token)
5. Telegram token collected (or skipped)
6. User ID collected (if token provided)
7. WhatsApp pairing via Yes/No toggle (or skipped)
8. Google Workspace setup (if gog installed, or skipped)
9. config.toml generated
10. System service install offer (or skipped)
11. Next steps + success outro
       |
[WIZARD ENDS]
       |
User reviews config.toml
       |
User runs: omega start
       |
Bot is running and listening on Telegram / WhatsApp
       |
User sends first message to bot
       |
Bot responds with Claude Code output
       |
Success: Omega is working
```

**Total time:** 2-5 minutes (mostly user input time, not waiting; longer if setting up Google Workspace)

---

## Why The Wizard Matters

### Without the Wizard
New users would face:
- Manual creation of `~/.omega` directory (confusion: "where should I put files?")
- Manual copy/edit of config file (risk of breaking TOML syntax)
- Manual lookup of how to create Telegram bot (external documentation required)
- Manual WhatsApp pairing setup
- Manual Google Workspace credential configuration
- Uncertainty: "Did I configure this right?"

**Result:** 15-30 minutes to get a working bot, high risk of misconfiguration

### With the Wizard
New users get:
- Beautiful cliclack-styled prompts (clear visual hierarchy with spinners, toggles, notes)
- Guided, interactive setup (clear prompts and instructions)
- Automatic directory and config generation (no manual file editing)
- Integrated help (links to @BotFather, @userinfobot, Google Cloud Console steps)
- Fast validation (Claude CLI check, credential file check, clear error messages)
- One-shot WhatsApp QR pairing
- Guided Google OAuth flow

**Result:** 2-5 minutes to get a fully working bot with all integrations, low risk of misconfiguration

---

## Error Handling During Onboarding

### Error: Claude CLI Not Found
**User sees:**
```
▲  claude CLI — NOT FOUND
│
│  Install claude CLI
│
│  npm install -g @anthropic-ai/claude-code
│
│  Then run 'omega init' again.
│
└  Setup aborted
```

**Why:** Claude Code is mandatory. Without it, Omega can't function. The wizard ends immediately with `outro_cancel`.

**User action:** Install npm package, re-run `omega init`

---

### Error: WhatsApp Pairing Timeout
**User sees:**
```
▲  Pairing did not complete
◇  You can try again later with /whatsapp.
```

**Why:** The user didn't scan the QR code within 60 seconds, or the WhatsApp server didn't confirm pairing.

**User action:** Continue with the wizard. WhatsApp will be disabled in config. The user can try again later.

---

### Error: Google Credentials File Not Found
**User sees:**
```
▲  File not found
```

**Why:** The path entered for `client_secret.json` doesn't exist after shell expansion.

**User action:** Re-enter the correct path. The input is validated and will keep asking until a valid path is provided.

---

### Error: Google OAuth Failed
**User sees:**
```
▲  gog auth add failed: <error details>
◇  Google Workspace setup incomplete.
```

**Why:** The OAuth flow in the browser was denied or timed out.

**User action:** Continue with the wizard. Google section will not be added to config. The user can set up Google manually later.

---

### Error: I/O Failure (Rare)
**User sees:** Rust error message from anyhow (e.g., "Permission denied" or "Disk full")

**Why:** Filesystem error when creating directory or writing config

**User action:** Fix filesystem issue (permissions, disk space), re-run `omega init`

---

### Error: Invalid TOML Written (Shouldn't Happen)
If there's a bug in the template, `config.toml` will be invalid. User would discover this when running `omega start`.

**Prevention:** The TOML template is hard-coded in `init.rs` and tested. The `generate_config` function is a pure function with unit tests covering full, minimal, and partial configurations.

---

## Customizing Omega After Init

After the wizard, users can customize Omega by editing `config.toml`:

**Change log level for debugging:**
```toml
log_level = "debug"  # More verbose logging
```

**Restrict tools Claude Code can use:**
```toml
allowed_tools = ["Read", "Write"]  # Remove Bash and Edit
```

**Add more Telegram users:**
```toml
allowed_users = [123456789, 987654321]  # Multiple user IDs
```

**Switch to different provider (when available):**
```toml
[provider]
default = "anthropic"  # or "openai", "ollama", etc.
```

**Increase context window:**
```toml
max_context_messages = 100  # Remember more history
```

**Add Google Workspace after init:**
```toml
[google]
account = "you@gmail.com"
```

After editing, restart Omega:
```bash
omega stop   # If running
omega start  # Restart with new config
```

---

## Resetting Omega to Fresh State

If the user wants to start over:

```bash
# Stop Omega if running
omega stop

# Delete config and data
rm config.toml
rm -rf ~/.omega

# Re-run setup wizard
omega init

# Start with new config
omega start
```

---

## Related Commands

### `omega start`
Starts the Omega daemon after init is complete. Loads config, initializes database, connects to Telegram and WhatsApp.

### `omega stop`
Stops the running Omega daemon. Gracefully shuts down all connections.

### `omega service`
(Separate from init) Registers Omega as a macOS LaunchAgent so it starts automatically on login. Not part of the init wizard; optional separate step.

### `omega ask`
(After Omega is running) Sends a message directly to Omega via CLI. Useful for testing without Telegram.

---

## Implementation Insights

### Why cliclack?

The wizard uses the `cliclack` crate instead of raw `println!`/`stdin` for several reasons:
- **Visual hierarchy:** `◆ ◇ │ ┌ └` markers make the flow scannable at a glance
- **Spinners:** Async operations (CLI checks, WhatsApp pairing, OAuth) show animated feedback
- **Confirm toggles:** Yes/No prompts are explicit toggles, not ambiguous text input
- **Input validation:** `cliclack::input().validate()` provides inline error messages without restarting the prompt
- **Notes:** Multi-line instructions (Google Cloud Console steps) are displayed in styled blocks
- **Consistent UX:** All Omega CLI interactions feel polished and professional

### Why Not Auto-Detect Telegram Token?
The wizard could theoretically:
- Check environment variables for `TELEGRAM_BOT_TOKEN`
- Read from a `.env` file
- Use OS keychain

**Decision:** Explicit prompt instead because:
- Force user to verify they have correct token
- Prevent accidental use of wrong token
- Keep wizard self-contained (no external file dependencies)
- Clear audit trail of what user configured

### Why Allow Skipping Telegram?
Some use cases don't need Telegram:
- Local CLI-only usage: `omega ask "your question"`
- WhatsApp-only usage
- Testing without live bot

**Decision:** Make token optional because:
- Users can test Omega locally without Telegram complexity
- Add Telegram to config.toml manually later
- Reduces setup friction for non-Telegram use cases

### Why Is Google Setup Conditional on `gog` CLI?
The Google Workspace step only appears if the `gog` CLI tool is already installed. This avoids:
- Confusing users who have no interest in Google integration
- Blocking the wizard on a non-essential dependency
- Unnecessary complexity for users who only want messaging

If `gog` is not installed, the wizard silently skips the entire Google section. The user never sees a question about it.

### Why Store Token in Config File?
Concern: Bot token in plaintext is a security risk.

**Current approach:** Token stored in `config.toml` (plaintext)

**Future improvement:** Support environment variables:
```bash
export TELEGRAM_BOT_TOKEN="123456:ABC..."
omega start
```

Then config.toml would reference the env var instead of token directly.

---

## Troubleshooting Common Issues

### "Command 'omega' not found"
**Problem:** Binary not built or not in PATH

**Solution:**
```bash
cargo build --release
# Now omega binary is at ./target/release/omega
# Either add to PATH or use full path: ./target/release/omega init
```

### "claude CLI not found"
**Problem:** Claude Code CLI not installed

**Solution:**
```bash
npm install -g @anthropic-ai/claude-code
```

### "config.toml already exists"
**Problem:** User wants to re-run setup wizard but config exists

**Solution:**
```bash
rm config.toml
omega init  # Generates new config
```

### "Failed to create ~/.omega directory"
**Problem:** Permission denied or disk full

**Solution:**
```bash
# Check permissions on home directory
ls -la ~

# Check disk space
df -h

# Try creating manually
mkdir -p ~/.omega
omega init  # Retry
```

### "Invalid bot token" (error during `omega start`)
**Problem:** Token was mistyped or copied incorrectly

**Solution:**
1. Get correct token from @BotFather again
2. Edit `config.toml` and update `bot_token = "..."`
3. Restart: `omega stop && omega start`

### "WhatsApp QR code timed out"
**Problem:** QR code didn't appear within 30 seconds

**Solution:**
- Ensure internet connectivity
- Try again: delete config.toml and re-run `omega init`
- Or pair manually later via the `/whatsapp` command

### "gog auth credentials failed"
**Problem:** The client_secret.json file is invalid or the `gog` tool cannot process it

**Solution:**
1. Re-download credentials from Google Cloud Console
2. Ensure the file is a valid OAuth client ID JSON (not a service account key)
3. Re-run `omega init` or configure manually with `gog auth credentials <path>`

---

## Design Philosophy

The init wizard embodies these principles:

### 1. **Guided, Not Opinionated**
The wizard guides users through necessary steps without forcing opinions on advanced customization. Users can edit `config.toml` afterward.

### 2. **Fail Fast, Fail Gracefully**
Critical dependencies (Claude CLI) are checked immediately with helpful error messages. Optional features (Telegram, WhatsApp, Google) can be skipped or fail without blocking the rest of the wizard.

### 3. **Minimize User Errors**
By generating config instead of asking users to edit templates, we eliminate syntax errors and misconfiguration. Input validation catches issues inline.

### 4. **Transparency**
Every step is visible to the user. No hidden operations. Users know what the wizard created and where. Spinners show when background work is happening.

### 5. **Completeness**
After the wizard, the system is fully functional. No additional setup required; user can immediately use Omega.

### 6. **Progressive Disclosure**
Optional integrations (Google Workspace) only appear when the prerequisites are met (gog CLI installed). Users are never overwhelmed with options that don't apply to them.

---

## Metrics of Success

The init wizard is successful if:

1. **New User Can Get Working Bot in 2-5 Minutes** -- Time target met
2. **No Surprises or Errors** -- Fast validation catches issues early
3. **User Understands What Was Configured** -- Explicit messages and next steps
4. **User Can Customize Later** -- Config.toml is documented and editable
5. **Bad State is Recoverable** -- User can delete and re-run
6. **Visual Polish** -- cliclack styling makes the experience professional and scannable

---

## Conclusion

The init wizard is the **entry point to Omega**. It transforms a raw codebase into a working personal AI agent in 2-5 minutes. Using `cliclack` for styled prompts, spinners, and toggles, the wizard provides a polished CLI experience that covers Telegram, WhatsApp, and Google Workspace setup in a single guided flow. By combining interactive guidance, automatic generation, and fast validation, the wizard removes friction while maintaining clarity and user control.

For new users, `omega init` is the bridge between "I found an interesting project" and "I have a fully connected AI agent."
