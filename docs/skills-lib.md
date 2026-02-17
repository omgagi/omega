# omega-skills — Developer Guide

## What is this crate?

`omega-skills` is a generic skill loader. It scans `~/.omega/skills/*/SKILL.md` for skill definitions and makes them available to the AI via the system prompt.

## How It Works

1. **Startup**: `install_bundled_skills(data_dir)` deploys core skills from the binary to `{data_dir}/skills/{name}/SKILL.md` (skips existing files)
2. **Migration**: `migrate_flat_skills(data_dir)` auto-migrates legacy flat `.md` files to the directory layout
3. **Load**: `load_skills(data_dir)` scans `{data_dir}/skills/` for subdirectories containing `SKILL.md`
4. **Frontmatter**: Each `SKILL.md` must have frontmatter between `---` delimiters (TOML or YAML — both work)
5. **Dep check**: Required CLI tools are checked via `which`
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

## Bot Command

`/skills` — Lists all loaded skills with their availability status.

## Bundled Skills

Core skills live in `skills/` at the repo root and are embedded into the binary at compile time via `include_str!`. On first startup (or after deletion), they are auto-deployed to `~/.omega/skills/{name}/SKILL.md`. User edits are never overwritten.

| Directory | Skill |
|-----------|-------|
| `google-workspace/SKILL.md` | Google Workspace CLI (`gog`) |

To add a new bundled skill: create the directory with a `SKILL.md` file in `skills/`, then add it to the `BUNDLED_SKILLS` const in `crates/omega-skills/src/lib.rs`.

## Migration

Legacy flat skill files (`~/.omega/skills/*.md`) are automatically migrated to the directory layout on startup. For each `foo.md`, a `foo/` directory is created and the file is moved to `foo/SKILL.md`. Existing directories are never overwritten.

## Projects

In addition to skills, the `omega-skills` crate also loads **projects** — user-defined instruction scopes.

### How Projects Work

1. Create a folder in `~/.omega/projects/` with any name (e.g., `real-estate`)
2. Add an `INSTRUCTIONS.md` file with custom instructions
3. Restart Omega
4. Use `/project real-estate` to activate it

When a project is active, its instructions are prepended to the system prompt, changing how the AI behaves.

### Project Directory Format

```
~/.omega/projects/
├── real-estate/
│   └── INSTRUCTIONS.md      # "You are a real estate analyst..."
├── nutrition/
│   └── INSTRUCTIONS.md      # "You are a nutrition coach..."
└── stocks/
    └── INSTRUCTIONS.md      # "You track my portfolio..."
```

### Bot Commands

- `/projects` — List all available projects, marking the active one
- `/project <name>` — Activate a project (clears conversation for clean context)
- `/project off` — Deactivate the current project
- `/project` — Show the currently active project

### Design Notes (Projects)

- **No frontmatter**: Unlike skills, projects are just plain markdown files. The directory name IS the project name.
- **Stored as fact**: The active project is stored as a user fact (`active_project`), so it persists across restarts.
- **Conversation cleared**: Switching projects closes the current conversation for a clean context.
- **No hot-reload**: Restart Omega to pick up new project folders.

## Design Notes

- **Lean prompt**: Only name + description go into the system prompt. The AI reads the full file on demand.
- **Bundled + user skills**: Core skills ship with the binary; users can add their own skill directories too.
- **No hot-reload**: Restart Omega to pick up new skill directories.
- **Install on demand**: All skills appear in the prompt regardless of install status. The AI can install missing tools by reading the skill file.
- **No per-skill Rust code**: The loader is fully generic — skills are just markdown files in directories.
