# Configuring Omega: A Step-by-Step Guide

Welcome! This guide walks you through setting up Omega for the first time using the `config.example.toml` template. We'll cover all the essential decisions and provide clear examples for common scenarios.

## What Is This Config File?

`config.example.toml` is a template showing all available Omega settings. You'll copy it to `config.toml` (which is private to you—never share it!) and customize it for your setup.

## Before You Start

Make sure you have:
- [ ] Omega repository cloned locally
- [ ] Cargo and Rust installed
- [ ] Python or Node.js (if using those providers)
- [ ] Approximately 5 minutes

## Step 1: Create Your Config File

```bash
cd ~/path/to/omega
cp config.example.toml config.toml
```

That's it! Now you have a private copy you can customize.

## Step 2: Set Your Bot's Identity

Open `config.toml` and look at the `[omega]` section:

```toml
[omega]
name = "Omega"
data_dir = "~/.omega"
log_level = "info"
```

**What to change:**
- **`name`:** The bot's display name. Use something memorable, e.g., `"MyAssistant"` or `"CodeBot"`.
- **`data_dir`:** Where Omega stores conversations and logs. `~/.omega` expands to your home directory—this is usually fine.
- **`log_level`:** How much detail to log. Use `"info"` for normal operation, `"debug"` if troubleshooting.

**Example:**
```toml
[omega]
name = "CodeBot"
data_dir = "~/.omega"
log_level = "info"
```

## Step 3: Choose Your AI Provider

Omega connects to an AI backend. You have five options; pick one.

### Option A: Claude Code (Recommended for New Users)

If you already use Claude Code locally (via `claude` CLI), this is your best choice. No API key needed!

```toml
[provider]
default = "claude-code"

[provider.claude-code]
enabled = true
max_turns = 10
allowed_tools = ["Bash", "Read", "Write", "Edit"]
timeout_secs = 600
```

**What this means:**
- `default = "claude-code"`: Use Claude Code as the primary AI.
- `enabled = true`: Turn it on.
- `max_turns = 10`: Conversations auto-summarize after 10 exchanges (prevents runaway context).
- `allowed_tools`: Let the AI use the `Bash`, `Read`, `Write`, `Edit` tools. Don't change this unless you know what you're doing.
- `timeout_secs = 600`: Max seconds to wait for the CLI to respond. 10-minute ceiling prevents runaway invocations.

**Setup:** Just ensure you have the `claude` CLI installed and authenticated:
```bash
which claude
```

If that returns a path, you're good. Otherwise, follow [Claude Code setup](https://github.com/anthropics/claude-code).

### Option B: Anthropic Sonnet (Recommended for API Users)

Use Anthropic's Claude API directly (paid, high-quality).

```toml
[provider]
default = "anthropic"

[provider.anthropic]
enabled = true
api_key = ""
model = "claude-sonnet-4-20250514"
```

**What to do:**
1. Create a free account at [console.anthropic.com](https://console.anthropic.com).
2. Generate an API key in Settings → API Keys.
3. **Don't put the key in `config.toml`!** Instead, set it as an environment variable:
   ```bash
   export ANTHROPIC_API_KEY="sk-ant-..."
   ```
4. Leave `api_key = ""` in the config file.

### Option C: OpenAI (GPT-4, ChatGPT)

For users who prefer OpenAI's models.

```toml
[provider]
default = "openai"

[provider.openai]
enabled = true
api_key = ""
model = "gpt-4o"
base_url = "https://api.openai.com/v1"
```

**Setup:**
1. Get an API key from [platform.openai.com](https://platform.openai.com/api-keys).
2. Set it as an environment variable:
   ```bash
   export OPENAI_API_KEY="sk-..."
   ```
3. Leave `api_key = ""` in the config.

### Option D: Ollama (Local, No Key)

Run a large language model locally using [Ollama](https://ollama.ai).

```toml
[provider]
default = "ollama"

[provider.ollama]
enabled = true
base_url = "http://localhost:11434"
model = "llama3"
```

**Setup:**
1. Install Ollama from [ollama.ai](https://ollama.ai).
2. Start the Ollama service:
   ```bash
   ollama serve
   ```
3. Pull your preferred model (in another terminal):
   ```bash
   ollama pull llama3
   ```
4. The config is ready—Ollama doesn't need an API key.

### Option E: OpenRouter (Multi-Provider)

Use OpenRouter to access hundreds of models (Anthropic, OpenAI, Meta, Mistral, etc.) with one API key.

```toml
[provider]
default = "openrouter"

[provider.openrouter]
enabled = true
api_key = ""
model = "anthropic/claude-sonnet-4-20250514"
```

**Setup:**
1. Sign up at [openrouter.ai](https://openrouter.ai).
2. Get your API key from the dashboard.
3. Set it as an environment variable:
   ```bash
   export OPENROUTER_API_KEY="sk-or-..."
   ```

---

**Quick Decision Tree:**
- Running locally? → Claude Code or Ollama
- Want high-quality? → Anthropic (Claude Sonnet)
- Prefer OpenAI? → OpenAI (GPT-4o)
- Want flexibility? → OpenRouter

For this guide, we'll assume **Claude Code**.

## Step 4: Enable Messaging Channels (Optional)

Omega can receive messages from Telegram or WhatsApp. You can skip this for now and just use the CLI.

### Using Telegram?

1. **Create a bot:**
   - Message [@BotFather](https://t.me/botfather) on Telegram.
   - Run `/newbot` and follow the prompts.
   - You'll get a `bot_token` like `123456789:ABCDEFGHIJKLMNop...`.

2. **Update config:**
   ```toml
   [channel.telegram]
   enabled = true
   bot_token = ""
   allowed_users = [YOUR_USER_ID]
   ```

3. **Find your Telegram user ID:**
   - Message the bot you just created.
   - Check the logs or use a service like [@userinfobot](https://t.me/userinfobot).
   - You'll get a numeric ID like `123456789`.

4. **Add your ID to the config:**
   ```toml
   allowed_users = [123456789]
   ```

5. **Set the token as an environment variable (secure):**
   ```bash
   export TELEGRAM_BOT_TOKEN="123456789:ABCDEFGHIJKLMNop..."
   ```

6. **Leave it empty in config:**
   ```toml
   bot_token = ""
   ```

### Using WhatsApp?

WhatsApp integration requires a bridge service, which is more complex. Skip for now unless you already have a WhatsApp bridge set up.

### For Now

If you're not sure, disable both channels:

```toml
[channel.telegram]
enabled = false

[channel.whatsapp]
enabled = false
```

You can always enable them later!

## Step 5: Configure Access Control (Security)

The `[auth]` section controls who can use Omega.

```toml
[auth]
enabled = true
deny_message = "Access denied. You are not authorized to use this agent."
```

**What this means:**
- `enabled = true`: Require users to be in an `allowed_users` list.
- `deny_message`: What to tell unauthorized users.

**Scenarios:**
- **Personal use:** Keep `enabled = true` and add only your Telegram/WhatsApp IDs to `allowed_users`.
- **Multi-user:** Set `enabled = true` and add all trusted user IDs.
- **Testing (not recommended for production):** Set `enabled = false` to allow anyone. Only do this temporarily!

## Step 6: Configure Memory (Conversation Storage)

The `[memory]` section handles conversation history.

```toml
[memory]
backend = "sqlite"
db_path = "~/.omega/memory.db"
max_context_messages = 50
```

**What this means:**
- `backend = "sqlite"`: Use the built-in SQLite database (no external setup needed).
- `db_path`: Where conversations are stored. `~/.omega` is the default—fine to leave it.
- `max_context_messages = 50`: When Omega answers, it includes the last 50 messages for context. More context = smarter answers but slower. Start with 50.

**Tuning:**
- **If responses are slow:** Lower to 25 or 30.
- **If Omega forgets context:** Raise to 75 or 100.

For a new installation, leave these at defaults.

## Step 7: Configure Security Sandbox

The `[sandbox]` section protects your system from accidental damage.

```toml
[sandbox]
enabled = true
allowed_commands = ["ls", "cat", "grep", "find", "git", "cargo", "npm", "python"]
blocked_paths = ["/etc/shadow", "/etc/passwd"]
max_execution_time_secs = 30
max_output_bytes = 1048576
```

**What this means:**
- **`enabled = true`:** Omega can only run commands in the `allowed_commands` list. This is a safety feature.
- **`allowed_commands`:** List of safe commands. The default set is good for development (git, cargo, npm, python, file reading).
- **`blocked_paths`:** Files Omega can never access (even via allowed commands). `/etc/shadow` and `/etc/passwd` are system files—don't change this.
- **`max_execution_time_secs = 30`:** Commands timeout after 30 seconds (prevents infinite loops).
- **`max_output_bytes = 1048576`:** Limit output to ~1 MB (prevents memory exhaustion).

**When to change:**
- **Adding more commands:** If you ask Omega to use a tool it doesn't have (e.g., `docker`), add it: `["ls", "cat", ..., "docker"]`.
- **Removing commands:** If you don't trust a provider with `npm`, remove it.

For now, keep defaults.

## Step 8: Configure the Scheduler (Task Queue)

The `[scheduler]` section controls the background task queue that delivers reminders and recurring tasks.

```toml
[scheduler]
enabled = true
poll_interval_secs = 60
```

**What this means:**
- **`enabled = true`:** The scheduler loop runs in the background, polling for due tasks. This is enabled by default and has zero overhead when no tasks exist.
- **`poll_interval_secs = 60`:** How often (in seconds) the scheduler checks for tasks that are due. 60 seconds is a good balance between responsiveness and database load.

**When to change:**
- If you want faster delivery for time-sensitive reminders, lower `poll_interval_secs` to `30` or `15`.
- If you want to disable scheduling entirely, set `enabled = false`.

**How tasks get created:**
You don't configure tasks here. Instead, you ask Omega in natural language:
- "Remind me to call John at 3pm"
- "Set a daily standup reminder at 9am"
- "Remind me every Monday to submit the report"

The AI provider translates these into structured tasks automatically.

## Step 9: Configure the Heartbeat (Optional)

The `[heartbeat]` section controls periodic AI check-ins. This is an advanced feature for proactive monitoring.

```toml
[heartbeat]
enabled = false
interval_minutes = 30
active_start = "08:00"
active_end = "22:00"
channel = "telegram"
reply_target = ""
```

**What this means:**
- **`enabled = false`:** Disabled by default. Enable it when you want proactive check-ins.
- **`interval_minutes = 30`:** How often the heartbeat fires (in minutes).
- **`active_start` / `active_end`:** Time window for heartbeats. Outside this window, heartbeats are skipped. Leave both empty for 24/7 operation.
- **`channel`:** Which messaging channel to send alerts on (e.g., `"telegram"`).
- **`reply_target`:** The platform-specific delivery target. For Telegram, this is the chat ID where alerts should be sent.

**To enable heartbeats:**
1. Set `enabled = true`.
2. Set `channel` to your active channel (e.g., `"telegram"`).
3. Set `reply_target` to your chat ID (the same ID you use for messaging Omega).
4. Optionally create `~/.omega/HEARTBEAT.md` with a checklist for the AI to evaluate.

**Example HEARTBEAT.md:**
```markdown
- Is disk usage below 90%?
- Are there any error logs in the last hour?
- Is the system load reasonable?
```

When everything is fine, the AI responds with `HEARTBEAT_OK` and no message is sent. When something needs attention, the AI sends an alert to your channel.

## Step 10: Set Environment Variables

For any API keys or bot tokens, use environment variables instead of hardcoding them.

**Example (Claude Code provider):**
No keys needed; you're done!

**Example (Anthropic provider):**
```bash
export ANTHROPIC_API_KEY="sk-ant-..."
```

**Example (Telegram):**
```bash
export TELEGRAM_BOT_TOKEN="123456789:ABCDEFGHIJKLMNop..."
```

**Make it persistent (macOS/Linux):**
Add to `~/.bashrc`, `~/.zshrc`, or `~/.profile`:
```bash
export ANTHROPIC_API_KEY="sk-ant-..."
export TELEGRAM_BOT_TOKEN="123456789:ABCDEFGHIJKLMNop..."
```

Then reload your shell:
```bash
source ~/.zshrc  # or ~/.bashrc
```

## Step 11: Validate and Start

**Check your config:**
```bash
cat config.toml | head -20  # Preview the file
```

**Start Omega:**

For CLI use (asking questions directly):
```bash
cargo build --release
./target/release/omega ask "What is 2 + 2?"
```

For 24/7 bot service (listens on Telegram):
```bash
./target/release/omega daemon
```

**Check the logs:**
```bash
tail -f ~/.omega/omega.log
```

## Common Configurations

### Scenario 1: Local Developer (Claude Code)

```toml
[omega]
name = "LocalBot"
data_dir = "~/.omega"
log_level = "debug"

[provider]
default = "claude-code"

[provider.claude-code]
enabled = true
max_turns = 10
allowed_tools = ["Bash", "Read", "Write", "Edit"]
timeout_secs = 600

[auth]
enabled = false  # Just me, no restrictions

[channel.telegram]
enabled = false

[memory]
backend = "sqlite"
db_path = "~/.omega/memory.db"
max_context_messages = 50

[sandbox]
enabled = true
allowed_commands = ["ls", "cat", "grep", "find", "git", "cargo", "npm", "python"]
blocked_paths = ["/etc/shadow", "/etc/passwd"]
max_execution_time_secs = 30
max_output_bytes = 1048576
```

### Scenario 2: Personal Telegram Bot (Anthropic API)

```toml
[omega]
name = "MyAssistant"
data_dir = "~/.omega"
log_level = "info"

[provider]
default = "anthropic"

[provider.anthropic]
enabled = true
api_key = ""  # Set ANTHROPIC_API_KEY env var
model = "claude-sonnet-4-20250514"

[auth]
enabled = true
deny_message = "Sorry, you're not authorized."

[channel.telegram]
enabled = true
bot_token = ""  # Set TELEGRAM_BOT_TOKEN env var
allowed_users = [123456789]  # Your user ID

[memory]
backend = "sqlite"
db_path = "~/.omega/memory.db"
max_context_messages = 50

[sandbox]
enabled = true
allowed_commands = ["ls", "cat", "grep", "find", "git", "cargo", "npm", "python"]
blocked_paths = ["/etc/shadow", "/etc/passwd"]
max_execution_time_secs = 30
max_output_bytes = 1048576
```

### Scenario 3: Multi-User Team (OpenRouter)

```toml
[omega]
name = "TeamBot"
data_dir = "~/.omega"
log_level = "info"

[provider]
default = "openrouter"

[provider.openrouter]
enabled = true
api_key = ""  # Set OPENROUTER_API_KEY env var
model = "anthropic/claude-sonnet-4-20250514"

[auth]
enabled = true
deny_message = "Access denied."

[channel.telegram]
enabled = true
bot_token = ""  # Set TELEGRAM_BOT_TOKEN env var
allowed_users = [123456789, 987654321, 555666777]  # Team member IDs

[memory]
backend = "sqlite"
db_path = "~/.omega/memory.db"
max_context_messages = 75  # More context for complex discussions

[sandbox]
enabled = true
allowed_commands = ["ls", "cat", "grep", "find", "git", "cargo", "npm", "python", "docker"]
blocked_paths = ["/etc/shadow", "/etc/passwd", "/root"]
max_execution_time_secs = 60  # Longer timeout for CI/CD tasks
max_output_bytes = 2097152  # 2 MB for verbose output
```

## Troubleshooting

### "config.toml not found"

```bash
cp config.example.toml config.toml
```

### "provider not enabled"

Check that your chosen provider has `enabled = true` and is set as `default`:

```toml
[provider]
default = "anthropic"

[provider.anthropic]
enabled = true  # <-- This must be true!
```

### "API key not found" or "API Error"

Make sure you exported the environment variable:

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
echo $ANTHROPIC_API_KEY  # Should print your key
```

**Common mistake:** Setting it in your terminal but not exporting it:
```bash
ANTHROPIC_API_KEY="sk-ant-..."  # Wrong (not exported)
export ANTHROPIC_API_KEY="sk-ant-..."  # Correct
```

### "Access denied" on Telegram

If auth is enabled, your user ID must be in `allowed_users`. Find it:

1. Send the bot a message.
2. Check the logs:
   ```bash
   grep "user_id" ~/.omega/omega.log | tail -5
   ```
3. Add it to config:
   ```toml
   allowed_users = [123456789]
   ```

### "Command not allowed"

If you ask Omega to use a tool it can't, add it to `allowed_commands`:

```toml
allowed_commands = ["ls", "cat", "grep", "find", "git", "cargo", "npm", "python", "docker"]
```

### "Database is locked"

Another Omega instance is running or crashed. Check:

```bash
ps aux | grep omega
```

Kill any stray processes:

```bash
pkill -f omega
```

If corrupted, delete the database (you'll lose history):

```bash
rm ~/.omega/memory.db
```

## Next Steps

1. **Read the full specification:** See `/specs/config-example-toml.md` for detailed technical information.
2. **Explore CLI options:** Run `omega --help` to see available commands.
3. **Set up persistent daemon:** Use your system's process manager (e.g., `systemd`, macOS LaunchAgent) to run Omega 24/7.
4. **Monitor logs:** Regularly check `~/.omega/omega.log` for errors and activity.
5. **Backup conversations:** Periodically back up `~/.omega/memory.db` if your conversations are valuable.

## Need Help?

- Check the project README for architecture and design.
- Read CLAUDE.md for design rules and constraints.
- Review the full spec: `/specs/config-example-toml.md`.
- Search the logs: `grep "ERROR\|WARN" ~/.omega/omega.log`.

Happy configuring!

