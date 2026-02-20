# OMEGA

**Your AI, your server, your rules.**

A personal AI agent infrastructure written in Rust. Omega connects to messaging platforms, delegates reasoning to configurable AI backends, and acts autonomously on your behalf. Single binary, no Docker, no cloud dependency.

## What Makes Omega Different

- **Autonomous, not assistive** -- Omega executes tasks, schedules follow-ups, and closes its own loops. It doesn't wait to be asked twice.
- **6 AI providers** -- Claude Code CLI, Anthropic API, OpenAI, Ollama, OpenRouter, Gemini. Swap with one config line.
- **Smart model routing** -- Every message is classified by complexity. Simple tasks use a fast model (Sonnet); complex work is decomposed into steps and executed by a powerful model (Opus). Automatic, no user intervention.
- **Real memory** -- SQLite-backed conversations, facts, summaries. Omega learns who you are across sessions.
- **OS-level sandbox** -- Seatbelt (macOS) / Landlock (Linux) filesystem enforcement. Not just prompt-based.
- **Self-healing** -- Detects infrastructure bugs, iterates on fixes with verification tests, escalates after 10 attempts.
- **Quantitative trading** -- Built-in Kalman filter, HMM regime detection, Kelly sizing, IBKR TWS integration with bracket orders and circuit breakers.
- **Runs locally** -- Your messages never touch third-party servers beyond the AI provider.

## Architecture

```
You (Telegram / WhatsApp)
        |
        v
  +-----------+     +----------------+     +-------------+
  |  Gateway   |---->|  AI Provider   |---->|   Response   |
  |            |     | (Claude Code,  |     |   + Markers  |
  |  Auth      |     |  Ollama, etc.) |     +------+------+
  |  Sanitize  |     +----------------+            |
  |  Classify  |                              +----v----+
  |  Route     |<-----------------------------| Process  |
  |  Audit     |      Memory (SQLite)         | Markers  |
  +-----------+      Facts, Summaries         +---------+
        |             Scheduled Tasks          SCHEDULE:
        v             Audit Log                SELF_HEAL:
  +----------+                                 LIMITATION:
  | Channels |                                 PROJECT_ACTIVATE:
  | Telegram |                                 LANG_SWITCH:
  | WhatsApp |                                 ...
  +----------+
```

Cargo workspace with 7 crates:

| Crate | Purpose |
|-------|---------|
| `omega-core` | Types, traits, config, error handling, prompt sanitization |
| `omega-providers` | 6 AI backends with unified `Provider` trait + agentic tool loop (bash/read/write/edit) + MCP client |
| `omega-channels` | Telegram (voice transcription, photo support) + WhatsApp (voice, images, groups, markdown) |
| `omega-memory` | SQLite storage, conversation history, facts, scheduled tasks, audit log, limitations |
| `omega-skills` | Skill loader with TOML/YAML frontmatter, project system, trigger-based MCP server activation |
| `omega-sandbox` | Seatbelt (macOS) / Landlock (Linux) filesystem enforcement with 3-level isolation |
| `omega-quant` | Standalone CLI -- Kalman filter, HMM, Kelly criterion, Merton allocation, IBKR TWS API |

## Quick Start

```bash
# Build (requires Rust nightly for WhatsApp dependency)
cargo +nightly build --release

# Interactive setup -- walks you through everything
./target/release/omega init

# Start
./target/release/omega start
```

Or manual setup:

```bash
cp config.example.toml config.toml   # Edit with your settings
./target/release/omega start
```

## How It Works

Every message flows through a deterministic pipeline:

1. **Dispatch** -- Concurrent per-sender. If you're already waiting for a response, new messages are buffered with an ack.
2. **Auth** -- Only allowed user IDs get through.
3. **Sanitize** -- Prompt injection patterns neutralized before reaching the AI.
4. **Context** -- Conversation history + user facts + active project + skills injected into system prompt.
5. **Classify** -- Fast model (Sonnet) decides: simple task = direct response, complex work = step-by-step plan.
6. **Route** -- Simple tasks handled by Sonnet. Complex tasks decomposed and executed by Opus with progress updates.
7. **Markers** -- AI emits protocol markers (`SCHEDULE:`, `SELF_HEAL:`, `LIMITATION:`, etc.) that the gateway processes and strips before delivery.
8. **Store** -- Exchange saved, conversation updated, facts extracted.
9. **Audit** -- Full interaction logged with model, tokens, processing time.
10. **Respond** -- Clean message sent back to user.

### Background Loops

- **Scheduler** -- Polls every 60s for due tasks. Reminders are delivered as text; action tasks invoke the AI with full tool access.
- **Heartbeat** -- Periodic health check with self-audit. Dynamic interval via `HEARTBEAT_INTERVAL:` marker.
- **Summarizer** -- Conversations idle for 30+ minutes are automatically summarized and closed.

### Marker Protocol

The AI communicates with the gateway through protocol markers emitted in response text:

| Marker | Purpose |
|--------|---------|
| `SCHEDULE: desc \| datetime \| repeat` | Schedule a reminder |
| `SCHEDULE_ACTION: desc \| datetime \| repeat` | Schedule an autonomous action |
| `PROJECT_ACTIVATE: name` | Activate a project context |
| `LANG_SWITCH: language` | Switch conversation language |
| `LIMITATION: title \| desc \| plan` | Report a capability gap |
| `SELF_HEAL: desc \| verification` | Initiate self-healing protocol |
| `HEARTBEAT_ADD: item` | Add item to monitoring checklist |

All markers are extracted, processed, and stripped before the response reaches the user.

## Providers

| Provider | Type | Auth | Notes |
|----------|------|------|-------|
| `claude-code` | CLI subprocess | Local `claude` auth | Default. Auto-resume on max_turns. MCP server injection. |
| `anthropic` | HTTP | `x-api-key` header | Direct Anthropic API with agentic tool loop |
| `openai` | HTTP | Bearer token | Works with any OpenAI-compatible endpoint |
| `ollama` | HTTP | None | Local models (llama3.1, mistral, etc.) |
| `openrouter` | HTTP | Bearer token | Access 100+ models via single API |
| `gemini` | HTTP | URL query param | Google Gemini API |

All HTTP providers include an agentic tool-execution loop (bash, read, write, edit) and MCP client support.

## Commands

| Command | Description |
|---------|-------------|
| `/status` | Uptime, provider, database info |
| `/memory` | Conversation and fact counts |
| `/history` | Last 5 conversation summaries |
| `/facts` | Known facts about you |
| `/forget` | Clear current conversation |
| `/tasks` | List scheduled tasks |
| `/cancel <id>` | Cancel a scheduled task |
| `/language` | Show or set language |
| `/personality` | Show or set behavior style |
| `/skills` | List available skills |
| `/projects` | List projects |
| `/project <name>` | Activate or deactivate a project |
| `/help` | Show all commands |

## Quantitative Trading (omega-quant)

Standalone CLI binary for quantitative analysis and trade execution via IBKR TWS:

```bash
omega-quant check                          # Test TWS connectivity
omega-quant scan --scan-code MOST_ACTIVE   # Market scanner
omega-quant analyze AAPL --bars 100        # Kalman + HMM + Kelly analysis
omega-quant order AAPL BUY 10 --stop-loss 1.5 --take-profit 3.0  # Bracket order
omega-quant positions                      # Open positions + P&L
omega-quant pnl DU1234567                  # Daily account P&L
omega-quant close AAPL                     # Close position
omega-quant orders                         # List open orders
omega-quant cancel 12345                   # Cancel an order
```

**Math pipeline**: Raw price -> Kalman filter (smooth + trend) -> HMM regime detection (Bull/Bear/Lateral) -> Merton optimal allocation -> Kelly fractional sizing -> Execution strategy (Immediate/TWAP).

**Safety**: Circuit breaker (2% deviation abort), daily limits (trades + USD), cooldown between trades, bracket orders (SL/TP), paper trading by default (port 4002).

## System Service

```bash
omega service install    # macOS LaunchAgent or Linux systemd (auto-start, restart on crash)
omega service status     # Check if running
omega service uninstall  # Remove
```

## Configuration

`config.toml` (gitignored):

```toml
[omega]
name = "Omega"
data_dir = "~/.omega"

[auth]
enabled = true

[provider]
default = "claude-code"

[provider.claude-code]
enabled = true
max_turns = 15
allowed_tools = ["Bash", "Read", "Write", "Edit"]

[provider.ollama]
enabled = true
base_url = "http://localhost:11434"
model = "llama3.1:8b"

[channel.telegram]
enabled = true
bot_token = "YOUR_TOKEN"
allowed_users = [123456789]

[memory]
db_path = "~/.omega/memory.db"
max_context_messages = 50
```

## Requirements

- Rust nightly (for WhatsApp dependency)
- `claude` CLI installed and authenticated (for default provider)
- Telegram bot token (from [@BotFather](https://t.me/BotFather))

## Development

```bash
cargo clippy --workspace     # Lint (zero warnings required)
cargo test --workspace       # All tests must pass
cargo fmt --check            # Formatting check
cargo build --release        # Optimized binary
```

## License

MIT
