# Documentation: Omega Self-Check

## Overview

The Omega Self-Check system is a startup diagnostic tool that verifies all critical components are operational before the gateway event loop begins. It provides immediate feedback on system readiness and helps diagnose configuration or installation issues early.

## What is Self-Check?

Self-Check is an async function that runs during Omega initialization, after configuration is loaded but before the main message processing loop starts. It:

- Verifies the SQLite memory database is accessible
- Confirms the configured AI provider is installed and working
- Validates enabled messaging channels (Telegram, WhatsApp, etc.)
- Produces styled diagnostic output using cliclack
- Signals overall system readiness (pass/fail)

## When to Run Self-Check

Self-Check is **automatically executed** during normal Omega startup:

1. **Every time Omega starts** -- the gateway calls `selfcheck::run()` before entering the event loop
2. **During development** -- catches configuration issues immediately
3. **After system changes** -- detects if dependencies were uninstalled or tokens revoked
4. **In production** -- confirms system health before handling messages

You can also manually trigger self-check using the `omega selfcheck` command:

```bash
omega selfcheck
```

## Example Output

Self-check uses [cliclack](https://crates.io/crates/cliclack) for styled terminal output. Results appear with a vertical bar layout, styled markers for pass/fail, and a summary footer.

### All Checks Pass

```
┌  omega self-check
│
◇  Database — accessible (2.5 MB)
◇  Provider — claude-code (available)
◇  Channel — telegram (@my_omega_bot)
│
└  All checks passed
```

### With Failures

```
┌  omega self-check
│
◇  Database — accessible (1.2 MB)
▲  Provider — claude-code (NOT FOUND — install claude CLI)
▲  Channel — telegram (token invalid — HTTP 401)
│
└  Some checks failed
```

## The Three Checks

### 1. Database Check

**What it validates:**
- SQLite memory database is accessible and working
- Database queries execute successfully
- Database file can be read and its size determined

**Where the database is:**
- Default location: `~/.omega/data/memory.db`
- Configured in: `config.toml` under `[storage]`

**What gets reported:**
- `◇  Database — accessible (2.5 MB)` -- Database is healthy
- `▲  Database — FAILED: {error}` -- Database cannot be accessed

**Why it might fail:**
- Database file is corrupted
- Permissions are wrong (file not readable by Omega process)
- Disk is full or unavailable
- SQLite library missing or incompatible

**How to fix it:**
```bash
# Check if database file exists and is readable
ls -la ~/.omega/data/memory.db

# If corrupted, backup and delete (will be recreated on next start)
mv ~/.omega/data/memory.db ~/.omega/data/memory.db.backup
```

---

### 2. Provider Check

**What it validates:**
- The configured AI provider is installed and available
- For claude-code: the `claude` CLI command is installed and in PATH

**Supported providers:**
- `claude-code` -- Claude AI via Anthropic CLI (checked)
- `anthropic` -- Direct Anthropic API (unchecked, requires runtime API key)
- `openai` -- OpenAI API (unchecked, requires runtime API key)
- `ollama` -- Local LLM (unchecked, requires runtime connectivity)
- `openrouter` -- OpenRouter API (unchecked, requires runtime API key)

**What gets reported:**
- `◇  Provider — claude-code (available)` -- claude CLI is installed
- `▲  Provider — claude-code (NOT FOUND — install claude CLI)` -- claude CLI not found
- `◇  Provider — anthropic (unchecked)` -- non-claude-code providers skip validation

**Why claude-code gets checked:**
- It's the default zero-config provider
- It's the only provider that requires local installation
- Other providers are checked at runtime when API calls are made

**How to fix it:**
```bash
# Install Claude CLI
npm install -g @anthropic-ai/claude

# Verify installation
which claude
claude --version

# Test it works
claude -p --output-format json
```

---

### 3. Channel Check (Telegram)

**What it validates:**
- Telegram bot is configured with a bot token
- Bot token is valid by verifying with Telegram API
- Bot account exists and is accessible

**Where the token comes from:**
- Configuration file: `config.toml` under `[channel.telegram]`
- Format: Alphanumeric string from BotFather (e.g., `123456789:ABCdefGHijklMNOpqrSTUvwxYZ`)

**Conditional execution:**
This check only runs if:
1. Telegram is configured in your `config.toml`
2. Telegram channel has `enabled = true`

If Telegram is not configured or disabled, this check is skipped.

**What gets reported:**
- `◇  Channel — telegram (@my_bot_username)` -- Bot token is valid and bot is accessible
- `▲  Channel — telegram (missing bot_token)` -- No token configured
- `▲  Channel — telegram (token invalid — HTTP 401)` -- Token rejected by Telegram
- `▲  Channel — telegram (network error: connection timeout)` -- Cannot reach Telegram servers

**Why it might fail:**
- Token is missing from config
- Token is invalid or expired
- Token is for a bot that was deleted in BotFather
- Network cannot reach `api.telegram.org`
- Firewall blocks Telegram API access
- Telegram service is temporarily down

**How to fix it:**
```bash
# Get or recreate a bot token from BotFather
# 1. Message @BotFather on Telegram
# 2. Type /newbot and follow instructions
# 3. Copy the token (format: 123456789:ABCdefGHijklMNOpqrSTUvwxYZ)

# Add to config.toml
[channel.telegram]
enabled = true
bot_token = "123456789:ABCdefGHijklMNOpqrSTUvwxYZ"

# Verify by testing the token
curl "https://api.telegram.org/bot123456789:ABCdefGHijklMNOpqrSTUvwxYZ/getMe"
```

---

## Interpreting Results

### All Checks Pass

```
┌  omega self-check
│
◇  Database — accessible (2.5 MB)
◇  Provider — claude-code (available)
◇  Channel — telegram (@my_omega_bot)
│
└  All checks passed
```

**Meaning:** System is fully operational. The gateway will start and handle messages normally.

---

### Database Check Fails

```
▲  Database — FAILED: unable to open database file
```

**Meaning:** Omega cannot access its memory store. Without a working database:
- Conversation history cannot be saved
- User preferences cannot be stored
- Audit logs cannot be written

**Impact:** System should not start. Database must be repaired.

---

### Provider Check Fails

```
▲  Provider — claude-code (NOT FOUND — install claude CLI)
```

**Meaning:** The Claude Code CLI is not installed. Omega cannot delegate AI reasoning to Claude.

**Impact:** System should not start. Install `claude` CLI first.

---

### Channel Check Fails

```
▲  Channel — telegram (token invalid — HTTP 401)
```

**Meaning:** The Telegram bot token is invalid. Omega cannot receive or send Telegram messages.

**Meaning if bot is configured:** Omega can still start but will not be able to process Telegram messages. Other channels (if configured) will work normally.

**Meaning if only Telegram is configured:** System starts but cannot handle any messages.

**Impact:** Fix the bot token or disable the Telegram channel.

---

## Return Status

The self-check function returns:
- **true** -- All checks passed, system is ready
- **false** -- One or more checks failed

### How Omega responds to `false`:
- During startup: The gateway may refuse to start or start in degraded mode
- The specific behavior depends on which checks failed
- Users should fix failures before the system is fully operational

## Step-by-Step Troubleshooting

### Problem: "Database — FAILED"

1. Check if database file exists:
   ```bash
   ls -la ~/.omega/data/memory.db
   ```

2. Check file permissions:
   ```bash
   # File should be readable/writable by current user
   chmod 600 ~/.omega/data/memory.db
   ```

3. Check disk space:
   ```bash
   df -h ~
   ```

4. If file is corrupted, rebuild it:
   ```bash
   rm ~/.omega/data/memory.db
   # Omega will recreate on next start
   ```

---

### Problem: "Provider — claude-code (NOT FOUND)"

1. Install the Claude CLI:
   ```bash
   npm install -g @anthropic-ai/claude
   ```

2. Verify installation:
   ```bash
   which claude
   claude --version
   ```

3. Test the CLI works:
   ```bash
   claude -p --output-format json <<< "say hello"
   ```

4. If installed but not found, check PATH:
   ```bash
   echo $PATH
   which claude
   ```

---

### Problem: "Channel — telegram (token invalid)"

1. Verify the token is correct:
   ```bash
   # Replace YOUR_TOKEN with your actual token
   curl "https://api.telegram.org/botYOUR_TOKEN/getMe"
   ```

   A valid token returns:
   ```json
   {"ok":true,"result":{"id":123456789,"is_bot":true,"first_name":"MyBot","username":"my_omega_bot",...}}
   ```

2. If invalid, get a new token from BotFather:
   - Message `@BotFather` on Telegram
   - Type `/newbot`
   - Follow instructions
   - Copy the token
   - Update `config.toml`

3. Verify Telegram API is reachable:
   ```bash
   curl "https://api.telegram.org/botYOUR_TOKEN/getMe"
   ```

---

### Problem: "Channel — telegram (network error)"

1. Check internet connectivity:
   ```bash
   ping -c 3 api.telegram.org
   ```

2. Check firewall rules:
   ```bash
   # On macOS
   sudo /usr/libexec/ApplicationFirewall/socketfilterfw --getglobalstate

   # Ensure Telegram is allowed
   sudo /usr/libexec/ApplicationFirewall/socketfilterfw --add /usr/bin/curl
   ```

3. Check for proxy requirements:
   ```bash
   # Ensure HTTP/HTTPS traffic can reach Telegram
   curl -v "https://api.telegram.org/botYOUR_TOKEN/getMe"
   ```

---

## Common Configurations

### Minimal (Claude Code only)

```toml
[provider]
default = "claude-code"

# No channels configured
```

**Self-check output:**
```
┌  omega self-check
│
◇  Database — accessible (0.1 MB)
◇  Provider — claude-code (available)
│
└  All checks passed
```

### With Telegram

```toml
[provider]
default = "claude-code"

[channel.telegram]
enabled = true
bot_token = "123456789:ABCdefGHijklMNOpqrSTUvwxYZ"
```

**Self-check output:**
```
┌  omega self-check
│
◇  Database — accessible (0.1 MB)
◇  Provider — claude-code (available)
◇  Channel — telegram (@my_bot)
│
└  All checks passed
```

### Disabled Telegram

```toml
[provider]
default = "claude-code"

[channel.telegram]
enabled = false
bot_token = "..."
```

**Self-check output:** (Telegram check is skipped)
```
┌  omega self-check
│
◇  Database — accessible (0.1 MB)
◇  Provider — claude-code (available)
│
└  All checks passed
```

---

## Performance

- **Database check:** Instant (< 10ms)
- **Provider check:** Fast (< 50ms, just binary existence check)
- **Telegram check:** Slowest (500ms-2000ms, depends on network)

Total self-check time: ~1-2 seconds with Telegram, ~50ms without.

---

## What Self-Check Does NOT Verify

Self-check is a lightweight startup validation. It does NOT check:

- Whether you have permission to send messages to specific Telegram chats
- Whether users in `allowed_users` config are valid Telegram IDs
- Whether the Claude API quota has been reached
- Whether firewall rules allow Omega to listen on configured ports
- Whether the LaunchAgent plist file has correct permissions
- Whether message delivery will succeed (only that auth works)
- Whether the database schema is up to date

These are runtime checks that happen as messages flow through the system.

---

## Integration with Omega Startup

```
Omega startup sequence:
1. Load config.toml
2. Connect to database
3. Initialize logging
4. Run self-check ← You are here
5. Start all channels (Telegram, WhatsApp, etc.)
6. Enter gateway event loop
```

If self-check fails, the gateway may:
- Exit with error status
- Start in degraded mode
- Print warnings to console
- Refuse to process certain types of messages

---

## For Developers

The self-check module is in `/Users/isudoajl/ownCloud/Projects/omega/src/selfcheck.rs`.

To add a new check:

1. Create a new `async fn check_xyz()` that returns `CheckResult`
2. Add it to the `run()` function's results vector
3. Include conditional logic if the check should only run sometimes

Example:

```rust
// New check function
async fn check_environment() -> CheckResult {
    // your validation logic
}

// In run()
results.push(check_environment().await);
```

---

## Summary

Self-Check is a quick, automatic health check that runs at startup. It verifies:

| Check | What | Why | How to Fix |
|-------|------|-----|-----------|
| **Database** | SQLite is accessible | Without it, Omega can't store state | Check file permissions or rebuild |
| **Provider** | Claude CLI (or other) is installed | Omega needs to delegate AI reasoning | Install claude CLI with npm |
| **Telegram** | Bot token is valid (if enabled) | Omega needs to receive/send messages | Get new token from BotFather |

All checks pass = System is ready to run. Any check fails = Fix that component before relying on Omega.
