# Understanding .gitignore

## Overview

Git's `.gitignore` file tells Git which files and directories to ignore when tracking changes. The Omega project uses `.gitignore` to prevent committing build artifacts, secrets, and runtime data that shouldn't be stored in version control.

## The Patterns Explained

### /target

```
/target
```

This ignores the `target` directory in the repository root. Cargo (Rust's package manager) creates this directory to store:

- Compiled binaries
- Intermediate compilation artifacts
- Dependency caches
- Debug symbols
- Test executables

**Why ignore it?**

- It's automatically regenerated every time you run `cargo build` or `cargo check`
- It's platform-specific (Linux binaries differ from macOS, Windows, etc.)
- It can grow to hundreds of megabytes
- Including it in Git causes merge conflicts and bloats the repository

**When it's created:**

```bash
cargo build          # Creates /target/debug/
cargo build --release  # Creates /target/release/
cargo test           # Creates test artifacts in /target/
cargo check          # Creates metadata in /target/
```

### *.db

```
*.db
```

This ignores any file ending in `.db` (database files). In the Omega project, this primarily applies to:

- `memory.db` — SQLite database storing conversation history, user facts, and audit logs

**Why ignore it?**

- Contains runtime state specific to each deployment
- Holds sensitive conversation data and personal information
- Changes frequently during normal operation
- Would create constant merge conflicts if committed
- Serves no purpose in version control

**Where it lives:**

According to `CLAUDE.md`, the database is stored at `~/.omega/data/memory.db`, outside the repository directory, so this rule is extra protection.

### config.toml

```
config.toml
```

This ignores the main configuration file. Users create `config.toml` from the template `config.example.toml` and add their own settings.

**Why ignore it?**

This is a **security-critical** rule. The `config.toml` file contains:

- Telegram bot token (for the Telegram channel)
- API keys for AI providers (Claude, Anthropic, OpenAI, OpenRouter, Ollama)
- User allowlists (who is authorized to use the bot)
- Custom provider credentials
- Personal information and preferences

**If committed, attackers could:**

- Use your bot tokens to impersonate the bot
- Access your API keys and drain your account credits
- Identify authorized users
- Modify your bot's behavior

**Correct workflow:**

1. Repository contains `config.example.toml` — a template with placeholder values
2. Each developer/deployment copies it:
   ```bash
   cp config.example.toml config.toml
   ```
3. They fill in their own secrets:
   ```toml
   # config.toml
   [telegram]
   bot_token = "123:ABC..."  # <- YOUR ACTUAL TOKEN

   [auth]
   allowed_users = ["your_username"]

   [provider_claude_code]
   enabled = true
   ```
4. Git ignores `config.toml`, so the template stays in the repo, but secrets stay local

### .env

```
.env
```

This ignores environment variable files. The `.env` file is used to set environment variables locally, typically for development.

**Why ignore it?**

Similar to `config.toml`, `.env` files contain sensitive information:

- API keys and tokens
- Database credentials
- Authentication secrets
- Private configuration values

**Example of what goes in .env:**

```bash
# .env (local, not committed)
ANTHROPIC_API_KEY=sk-...
OPENAI_API_KEY=sk-...
TELEGRAM_BOT_TOKEN=...
DATABASE_URL=sqlite:memory.db
DEBUG=true
```

**Correct workflow:**

1. Create a template file `.env.example` (committed, with dummy values)
2. Each developer creates `.env` locally (not committed)
3. Load variables with:
   ```bash
   set -a
   source .env
   set +a
   ```

**Note:** The current Omega project primarily uses `config.toml`, but `.env` support is included for flexibility with alternative providers and future extensibility.

## How Git Uses .gitignore

### Checking if a file is ignored

```bash
# Shows the .gitignore rule that matches a file
git check-ignore -v config.toml
# Output: .gitignore:3:config.toml    config.toml

# The format is: <pattern>:<line>:<path>    <file>
```

### Viewing all ignored files

```bash
git status --ignored
# Shows all files that match .gitignore patterns
```

### Forcing Git to track an ignored file (not recommended)

```bash
git add -f config.toml  # Force add, overrides .gitignore
```

This is **NOT recommended** for secrets!

## Pattern Syntax Reference

`.gitignore` uses glob patterns:

| Pattern | Meaning |
|---------|---------|
| `file.txt` | Ignore file in any directory |
| `*.log` | Ignore all `.log` files |
| `/target` | Ignore `/target` at repo root only |
| `target/` | Ignore `target` directory (with trailing slash) |
| `*.db` | Ignore all `.db` files anywhere |
| `!important.db` | Exception: Don't ignore this `.db` file |
| `**/*.tmp` | Ignore `.tmp` files at any depth |
| `.env*` | Ignore `.env`, `.env.local`, `.env.production`, etc. |

## Security Best Practices

### The Golden Rule

**Never commit secrets to Git.** Secrets in version control history are **permanently exposed**, even if you delete them later.

### For the Omega Project

1. **config.toml:** Always gitignored. Manage via `config.example.toml` template.
2. **.env:** Always gitignored. Create locally if using environment variables.
3. **memory.db:** Always gitignored. Stored in `~/.omega/` outside the repo.
4. **Treat `.gitignore` as security infrastructure:** Review it carefully before committing.

### If You Accidentally Commit a Secret

1. Immediately revoke the exposed key/token in the service
2. Remove it from history with `git filter-repo` or `git filter-branch`
3. Force-push the cleaned history (discuss with team first)
4. Update your gitignore patterns to prevent recurrence

## Omega-Specific Considerations

From `CLAUDE.md`, the Omega project has these key configuration points:

- **config.toml:** Primary configuration (must be gitignored)
- **config.example.toml:** Template for users to start from (must be committed)
- **~/.omega/data/memory.db:** SQLite database storing state (excluded by `*.db` pattern)
- **~/.omega/logs/omega.log:** Application logs (not currently in .gitignore, but could be added)
- **~/Library/LaunchAgents/com.omega-cortex.omega.plist:** Service file (user-specific, not in repo)

The `.gitignore` correctly protects all sensitive areas of this architecture.

## Troubleshooting

### "I can't commit my config.toml changes!"

This is intentional. The `.gitignore` is protecting your secrets. Use `config.example.toml` instead to communicate configuration structure:

```bash
# Edit the template
vim config.example.toml

# Commit the template (no secrets)
git add config.example.toml
git commit -m "docs: update config template"

# Your local config.toml changes stay local
```

### "A file should be ignored but it's not"

Check if there's a local exception:

```bash
git check-ignore -v myfile
```

Or add a pattern to `.gitignore` and commit it:

```bash
echo "*.tmp" >> .gitignore
git add .gitignore
git commit -m "chore: ignore tmp files"
```

### "I need to share environment variables with my team"

Use an example file:

```bash
cp .env .env.example
# Edit .env.example to remove secrets
git add .env.example
git commit -m "docs: add .env template"
```

## Further Reading

- [Git Documentation: gitignore](https://git-scm.com/docs/gitignore)
- [GitHub: A collection of .gitignore templates](https://github.com/github/gitignore)
- [Rust-specific .gitignore patterns](https://github.com/github/gitignore/blob/main/Rust.gitignore)
