# User Documentation: Ω OMEGA Commands

## What Are Commands?

Commands are special messages that start with a forward slash (`/`) and provide instant responses from Omega. Unlike regular messages that are sent to Claude for reasoning, commands are handled immediately by Omega itself—no AI processing needed.

**Key Difference:**
- **Regular Message:** "What's the capital of France?" → Sent to Claude → Response: "The capital of France is Paris."
- **Command:** `/status` → Handled locally → Response: "Ω OMEGA Status, Uptime: 2h 15m 30s, ..."

---

## Available Commands

### `/status` — System Status

**What It Does:** Shows Omega's current operational status, including how long it has been running, which AI provider is active, and database size.

**Response Example:**
```
*Ω OMEGA* Status
Uptime: 2h 15m 30s
Provider: Claude Code CLI
Sandbox: sandbox
Database: 1.4 MB
```

**Use Cases:**
- Check if Omega is responsive and healthy
- Monitor database growth over time
- Verify which AI provider is configured
- Confirm the active sandbox mode (sandbox, rx, or rwx)

---

### `/memory` — Your Memory Stats

**What It Does:** Displays statistics about your personal memory stored in Omega, including the number of conversations, messages exchanged, and facts known about you.

**Response Example:**
```
Your Memory
Conversations: 7
Messages: 52
Facts: 4
```

**Understanding the Numbers:**
- **Conversations:** How many distinct conversation threads you've had
- **Messages:** Total number of messages you've sent to Omega across all conversations
- **Facts:** Pieces of information Omega has learned and remembered about you (e.g., your name, preferences, location)

**Use Cases:**
- Check how much interaction history Omega has
- See how many facts have been extracted about you
- Understand memory storage accumulation

---

### `/history` — Conversation History

**What It Does:** Shows summaries of your last 5 conversations with timestamps, allowing you to recall previous topics without viewing full message logs.

**Response Example:**
```
Recent Conversations

[2025-02-16 14:30:15]
Discussed project architecture and design patterns for Rust microservices

[2025-02-16 13:15:22]
Reviewed async/await best practices and error handling strategies

[2025-02-16 11:45:00]
Troubleshooted database schema migration issues
```

**Use Cases:**
- Quickly recall what you discussed previously
- Return to a previous conversation topic
- Verify that conversations were properly closed

---

### `/facts` — Known Facts About You

**What It Does:** Lists all the facts Omega has learned and stored about you during conversations. Facts are extracted automatically when you share personal information.

**Response Example:**
```
Known Facts

- favorite_language: Rust
- location: San Francisco Bay Area
- timezone: Pacific Standard Time
- job_title: Senior Software Engineer
```

**Understanding Facts:**
- Facts are automatically extracted from your messages
- They help Omega provide personalized responses
- You can ask Omega to forget facts or update them

**Use Cases:**
- Verify what information Omega has about you
- Ensure privacy by checking stored facts
- Confirm accurate information is being used for context

---

### `/forget` — Clear Current Conversation

**What It Does:** Clears the current active conversation, allowing you to start fresh without the previous context. This is useful when you want to begin a completely new topic or restart Omega's memory for the conversation.

**Response Example (Success):**
```
Conversation cleared. Starting fresh.
```

**Response Example (No Active Conversation):**
```
No active conversation to clear.
```

**Important Notes:**
- `/forget` only affects the current conversation, not your entire memory
- Previous conversations remain in your history
- Facts learned about you are preserved
- The next message you send will start a new conversation

**Use Cases:**
- Start a completely different topic
- Reset context when conversations become off-track
- Reduce token usage by avoiding large context windows
- Begin a focused discussion without previous distractions

---

### `/tasks` — Scheduled Tasks

**What It Does:** Lists all your pending scheduled tasks, showing a short ID, description, due date, and repeat type for each.

**Response Example:**
```
Scheduled Tasks

[a1b2c3d4] Call John
  Due: 2026-02-17T15:00:00 (once)

[e5f6g7h8] Stand-up meeting
  Due: 2026-02-18T09:00:00 (daily)
```

**Response Example (No Tasks):**
```
No pending tasks.
```

**Understanding the Output:**
- The 8-character string in brackets (e.g., `[a1b2c3d4]`) is the short ID prefix of the task's UUID. Use it with `/cancel` to remove the task.
- **Due** shows when the task will next fire.
- The parenthesized label shows the repeat type: `once`, `daily`, `weekly`, `monthly`, or `weekdays`.

**Use Cases:**
- Review what reminders you have pending
- Get the short ID needed to cancel a task
- Check the next delivery time for recurring tasks

---

### `/cancel` — Cancel a Scheduled Task

**What It Does:** Cancels a pending scheduled task by its short ID prefix. The task must belong to you and must still be in `pending` status.

**Response Example (Success):**
```
Task cancelled.
```

**Response Example (No Match):**
```
No matching task found.
```

**Response Example (No ID Provided):**
```
Usage: /cancel <task-id>
```

**How to Use:**
1. Run `/tasks` to see your pending tasks and their short IDs.
2. Copy the 8-character ID (e.g., `a1b2c3d4`).
3. Send `/cancel a1b2c3d4`.

**Important Notes:**
- You can only cancel your own tasks.
- Cancelled tasks are not deleted -- they remain in the database with `status = 'cancelled'` for audit purposes.
- If a task has already been delivered, it cannot be cancelled.

**Use Cases:**
- Remove a reminder you no longer need
- Stop a recurring task (e.g., a daily standup reminder you set up temporarily)
- Correct a scheduling mistake before the task fires

---

### `/language` (or `/lang`) — Language Preference

**What It Does:** Shows or sets your preferred response language. Omega auto-detects your language on your first message, but you can override it at any time.

**Response Example (Show Current):**
```
Language: Spanish
Usage: /language <language>
```

**Response Example (Set New):**
```
Language set to: French
```

**Use Cases:**
- Override the auto-detected language
- Switch Omega's response language without asking in-chat
- Check what language Omega currently uses for you

**How It Works:**
- On your first message, Omega detects your language and stores it as a preference.
- All future conversations use that preference until you change it.
- You can also ask Omega to "speak in French" in a regular message, and it will switch automatically.

---

### `/projects` — List Projects

**What It Does:** Lists all available projects in `~/.omega/projects/`, marking which one is currently active.

**Response Example:**
```
Projects

- real-estate (active)
- nutrition
- stocks

Use /project <name> to activate, /project off to deactivate.
```

**Response Example (No Projects):**
```
No projects found. Create folders in ~/.omega/projects/ with INSTRUCTIONS.md
```

**Use Cases:**
- See what projects are available
- Check which project is currently active
- Get the exact project name for activation

---

### `/project` — Manage Active Project

**What It Does:** Shows, activates, or deactivates the current project.

**Usage:**
- `/project` — Show the current active project
- `/project <name>` — Activate a project
- `/project off` — Deactivate the current project

**Response Example (Show Current):**
```
Active project: real-estate
Use /project off to deactivate.
```

**Response Example (Activate):**
```
Project 'real-estate' activated. Conversation cleared.
```

**Response Example (Deactivate):**
```
Project deactivated. Conversation cleared.
```

**Important Notes:**
- Switching projects clears your current conversation for a clean context
- The active project persists across Omega restarts
- Project instructions are prepended to the system prompt, changing how the AI behaves
- To add a new project, create a folder in `~/.omega/projects/` with an `INSTRUCTIONS.md` file and restart Omega

**Use Cases:**
- Scope AI behavior for specific domains (real estate, nutrition, finance)
- Switch between different "AI personas" on the fly
- Deactivate when you want the default AI behavior back

---

### `/help` — Command Help

**What It Does:** Displays a quick reference guide of all available commands with brief descriptions.

**Response Example:**
```
*Ω OMEGA* Commands

/status   — Uptime, provider, database info
/memory   — Your conversation and facts stats
/history  — Last 5 conversation summaries
/facts    — List known facts about you
/forget   — Clear current conversation
/tasks    — List your scheduled tasks
/cancel   — Cancel a task by ID
/language — Show or set your language
/projects — List available projects
/project  — Show, activate, or deactivate a project
/help     — This message
```

**Use Cases:**
- Quick reference when you forget a command
- Onboarding new users to Omega
- Learn available commands at any time

---

## How Commands Differ from Regular Messages

### Regular Messages
- Start with any character except `/`
- Sent to Claude (or configured AI provider)
- Process through full reasoning pipeline
- May take 5-30+ seconds
- Consume API credits (if using paid providers)
- Context includes conversation history and facts
- Example: "What's the weather today?"

### Commands
- Must start with `/` followed by command name
- Handled instantly by Omega itself
- No AI provider involved
- Response in milliseconds
- Zero API cost
- No context dependencies
- Example: `/status`

---

## Command Behavior

### Per-User & Per-Channel Isolation

Each command operates within your user account and messaging channel context:
- `/memory` shows only *your* stats, not other users'
- `/forget` only clears *your* current conversation in that channel
- `/facts` displays only facts about *you*
- `/history` shows only *your* conversations

If you use Omega across multiple channels (e.g., Telegram and WhatsApp), your memory is shared but conversation states are per-channel.

---

## Error Messages

If a command encounters an error (e.g., database issue), you'll see:
```
Error: [description]
```

Common error scenarios:
- Database connection issues (rare, usually temporary)
- Corrupted memory data (very rare, auto-recovers)

If errors persist, check with the Omega administrator or review logs.

---

## Tips and Best Practices

### Use `/forget` Strategically
- Use when switching between unrelated topics
- Reduces context window size for faster responses
- Keeps conversations focused and organized

### Monitor `/memory` Growth
- Periodically check `/memory` to understand your usage
- Large message counts don't hurt performance, but be aware of privacy implications
- Facts are meant to improve personalization—verify they're accurate with `/facts`

### Refer to `/history` for Context
- Before asking follow-up questions, check `/history` to understand past conversations
- Helps write better prompts that reference previous work

### Quick System Checks
- Use `/status` to verify Omega is responsive
- Monitor database size to ensure storage isn't growing unexpectedly

---

## Troubleshooting

### "No active conversation to clear"
This appears when you run `/forget` but haven't sent any messages yet in the current conversation. Simply start a conversation first by sending a message.

### "/unknown" or unknown command treated as regular message
Commands are case-sensitive. Use lowercase: `/status`, not `/Status`. Unknown commands (e.g., `/xyz`) are passed to Claude as regular messages.

### Response shows "Error: ..."
A temporary issue with memory storage. Usually resolves on retry. Contact support if persistent.

---

## What's Next?

Want to learn more?
- **Omega Overview:** See main documentation for architecture and setup
- **Memory System:** Details on how facts and conversation history work
- **Conversation Flow:** Understanding how messages are processed through Omega

