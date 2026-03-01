# omega-skills -- Developer Guide

## What is this crate?

`omega-skills` is a generic skill and project loader. It scans `~/.omega/skills/*/SKILL.md` for skill definitions and `~/.omega/projects/*/ROLE.md` for project definitions, making them available to the AI via the system prompt. It also handles MCP (Model Context Protocol) server matching via trigger keywords.

### Dependencies

- `omega-core` -- shared types including `McpServer`

## How It Works

1. **Startup**: `install_bundled_skills(data_dir)` deploys core skills from the binary to `{data_dir}/skills/{name}/SKILL.md` (skips existing files)
2. **Migration**: `migrate_flat_skills(data_dir)` auto-migrates legacy flat `.md` files to the directory layout
3. **Load**: `load_skills(data_dir)` scans `{data_dir}/skills/` for subdirectories containing `SKILL.md`
4. **Frontmatter**: Each `SKILL.md` must have frontmatter between `---` delimiters (TOML or YAML -- both work)
5. **Dep check**: Required CLI tools are checked via a pure-Rust `$PATH` search (no subprocess spawned)
6. **Prompt**: `build_skill_prompt()` builds a block appended to the system prompt listing all skills with their install status and file path
7. **On demand**: When the AI needs a skill, it reads the full `SKILL.md` file for instructions

## Skill Directory Format

Create a directory in `~/.omega/skills/` with a `SKILL.md` file:

```
~/.omega/skills/
├── google-workspace/
│   └── SKILL.md
└── my-custom-tool/
    └── SKILL.md
```

TOML format (our convention):
```markdown
---
name = "gog"
description = "Google Workspace CLI."
requires = ["gog"]
homepage = "https://gogcli.sh"
---

# Full usage instructions here
```

YAML format (third-party skills, e.g. npm packages):
```markdown
---
name: playwright-mcp
description: Browser automation via Playwright MCP.
requires: [npx, playwright-mcp]
homepage: https://playwright.dev
---

# Full usage instructions here
```

The parser tries TOML first, then falls back to YAML-style `key: value` parsing. It also extracts `requires` from openclaw-style `metadata` JSON blobs when present.

### Frontmatter Fields

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Short identifier |
| `description` | Yes | One-line description for the AI |
| `requires` | No | List of CLI tools that must be on `$PATH` |
| `homepage` | No | URL for reference |
| `trigger` | No | Pipe-separated keywords for MCP activation (e.g. `"browser\|web\|scrape"`) |
| `mcp` (TOML) / `mcp-<name>` (YAML) | No | MCP server definitions (see below) |

### MCP Server Definitions

Skills can declare MCP servers that are activated when the skill's trigger keywords match an incoming message. There are two syntax variants depending on frontmatter format:

**TOML format** -- use a `[mcp.<name>]` table:

```markdown
---
name = "playwright-mcp"
description = "Browser automation via Playwright MCP."
requires = ["npx"]
homepage = "https://playwright.dev"
trigger = "browser|web|scrape|screenshot|crawl"

[mcp.playwright]
command = "npx"
args = ["@anthropic/playwright-mcp"]
---

# Full usage instructions here
```

**YAML format** -- use the `mcp-<name>` shorthand:

```markdown
---
name: playwright-mcp
description: Browser automation via Playwright MCP.
requires: [npx]
homepage: https://playwright.dev
trigger: browser|web|scrape|screenshot|crawl
mcp-playwright: npx @anthropic/playwright-mcp
---

# Full usage instructions here
```

Both formats produce a `Skill` struct with `trigger: Option<String>` and `mcp_servers: Vec<McpServer>` populated accordingly.

### MCP Command Validation

All MCP command names are validated before acceptance. Only these characters are allowed: alphanumeric, hyphens (`-`), underscores (`_`), dots (`.`), forward slashes (`/`), and at-signs (`@`). Shell metacharacters (`;`, `|`, `&`, `$`, backticks, `>`, `<`, `(`, `)`, spaces, etc.) are rejected. Empty commands are also rejected.

This prevents command injection via malicious skill files. Rejected commands are logged with a warning and the MCP server entry is silently dropped.

## MCP Trigger Matching

The public function `match_skill_triggers(skills, message) -> Vec<McpServer>` scans all loaded skills for trigger keyword matches against the incoming message:

1. For each skill with a `trigger` field, splits the value on `|` to get individual keywords.
2. Performs case-insensitive substring matching against the message text.
3. Skips skills that are not available (i.e., their required CLI tools are missing).
4. Collects all `mcp_servers` from matched skills.
5. Deduplicates by server name -- if two skills declare the same MCP server, it appears only once.

The returned `Vec<McpServer>` is set on the `Context` before it is passed to the provider.

## Bot Command

`/skills` -- Lists all loaded skills with their availability status.

## Bundled Skills

Core skills live in `skills/` at the repo root and are embedded into the binary at compile time via `include_str!`. On first startup (or after deletion), they are auto-deployed to `~/.omega/skills/{name}/SKILL.md`. User edits are never overwritten.

| Directory | Skill |
|-----------|-------|
| `claude-code/SKILL.md` | Claude Code CLI (`claude`) |
| `google-workspace/SKILL.md` | Google Workspace CLI (`gog`) |
| `playwright-mcp/SKILL.md` | Browser automation via Playwright MCP (`npx`) |
| `skill-creator/SKILL.md` | Skill creator (meta-skill for creating new skills) |
| `ibkr-trader/SKILL.md` | Interactive Brokers trading via omega-trader binary |

To add a new bundled skill: create the directory with a `SKILL.md` file in `skills/`, then add it to the `BUNDLED_SKILLS` const in `backend/crates/omega-skills/src/skills.rs`.

## Migration

Legacy flat skill files (`~/.omega/skills/*.md`) are automatically migrated to the directory layout on startup. For each `foo.md`, a `foo/` directory is created and the file is moved to `foo/SKILL.md`. Existing directories are never overwritten.

## Security

### Path Traversal Protection

Both `load_skills()` and `load_projects()` canonicalize directory entries via `std::fs::canonicalize()` and verify the resolved path is still under the expected parent directory using `Path::starts_with()` (component-aware, not string prefix matching). Symlinks that escape the parent directory are blocked with a warning log.

### Dependency Checking

The `which_exists()` function uses a pure-Rust `$PATH` search instead of spawning a `which` subprocess. It iterates each directory in the `PATH` environment variable and checks for file existence. This avoids blocking I/O in the async runtime.

## Projects

In addition to skills, the `omega-skills` crate also loads **projects** -- user-defined instruction scopes.

### How Projects Work

1. Create a folder in `~/.omega/projects/` with any name (e.g., `real-estate`)
2. Add a `ROLE.md` file with custom instructions (optionally with frontmatter)
3. Restart Omega
4. Use `/project real-estate` to activate it

When a project is active, its instructions are prepended to the system prompt, changing how the AI behaves.

### Project Directory Format

```
~/.omega/projects/
├── real-estate/
│   └── ROLE.md      # "You are a real estate analyst..."
├── nutrition/
│   └── ROLE.md      # "You are a nutrition coach..."
└── stocks/
    └── ROLE.md      # "You track my portfolio..."
```

### Project Frontmatter

ROLE.md files support optional frontmatter between `---` delimiters for declaring skill dependencies. The parser tries TOML first, with YAML fallback.

**TOML format:**
```markdown
---
skills = ["ibkr-trader", "playwright-mcp"]
---

You are a trading assistant. Use the IBKR trader skill for portfolio management.
```

**YAML format:**
```markdown
---
skills: [ibkr-trader, playwright-mcp]
---

You are a trading assistant. Use the IBKR trader skill for portfolio management.
```

**No frontmatter (backward compatible):**
```markdown
You are a real estate analyst. Help me evaluate property deals.
```

| Frontmatter Field | Required | Default | Description |
|-------------------|----------|---------|-------------|
| `skills` | No | `[]` | List of skill names this project depends on |

When frontmatter is present, the body after the closing `---` is used as the project instructions. When absent, the entire file content is used.

### Bot Commands

- `/projects` -- List all available projects, marking the active one
- `/project <name>` -- Activate a project (clears conversation for clean context)
- `/project off` -- Deactivate the current project
- `/project` -- Show the currently active project

### Design Notes (Projects)

- **Optional frontmatter**: Projects support TOML/YAML frontmatter for declaring skill dependencies, but it is not required. Plain markdown files work fine for backward compatibility.
- **Stored as fact**: The active project is stored as a user fact (`active_project`), so it persists across restarts.
- **Conversation cleared**: Switching projects closes the current conversation for a clean context.
- **No hot-reload**: Restart Omega to pick up new project folders.
- **Path traversal protection**: Project directories are canonicalized and verified to be under `{data_dir}/projects/`.

## Design Notes

- **Lean prompt**: Only name + description go into the system prompt. The AI reads the full file on demand.
- **Bundled + user skills**: Core skills ship with the binary; users can add their own skill directories too.
- **No hot-reload**: Restart Omega to pick up new skill directories.
- **Install on demand**: All skills appear in the prompt regardless of install status. The AI can install missing tools by reading the skill file.
- **No per-skill Rust code**: The loader is fully generic -- skills are just markdown files in directories.
- **MCP command validation**: All MCP command names are validated to prevent shell injection from malicious skill files.
- **Path traversal guards**: Both skill and project loaders canonicalize paths and verify containment to prevent directory escape via symlinks.
