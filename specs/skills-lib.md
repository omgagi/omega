# Technical Specification: `omega-skills/src/lib.rs`

## File

| Field | Value |
|-------|-------|
| **Path** | `crates/omega-skills/src/lib.rs` (re-export orchestrator) |
| **Crate** | `omega-skills` |
| **Role** | Generic skill loader — scans `~/.omega/skills/*/SKILL.md` and exposes them to the system prompt |

## Module Structure

The crate is split into 3 focused submodules; `lib.rs` is a thin re-export orchestrator:

| File | Responsibility |
|------|---------------|
| `lib.rs` | `mod` declarations + `pub use` re-exports — all consumers use `omega_skills::*` paths |
| `skills.rs` | Skill loading, parsing, bundled install, migration, trigger matching, MCP activation |
| `projects.rs` | Project loading, parsing, `ensure_projects_dir`, `get_project_instructions` |
| `parse.rs` | Shared `pub(crate)` parsing utilities: `expand_tilde`, `unquote`, `parse_yaml_list`, `extract_bins_from_metadata`, `which_exists`, `data_path` |

## Purpose

Loads skill definitions from `SKILL.md` files inside per-skill directories. Frontmatter can be TOML (`key = "value"`) or YAML (`key: value`) — the parser tries TOML first, then falls back to YAML-style parsing so skill files from any source (npm packages, third parties) just work. Each skill file declares a name, description, required CLI tools, and optional homepage. The loader checks whether required tools are installed and builds a prompt block that tells the AI what skills exist and where to read full instructions.

## Public API

| Item | Kind | Description |
|------|------|-------------|
| `Skill` | struct | Loaded skill definition (name, description, requires, homepage, available, path, trigger, mcp_servers) |
| `install_bundled_skills(data_dir)` | fn | Deploy bundled core skills to `{data_dir}/skills/{name}/SKILL.md`, creating subdirs if needed. Never overwrites existing files. |
| `migrate_flat_skills(data_dir)` | fn | Auto-migrate legacy flat `.md` files to `{name}/SKILL.md` directory layout. Skips if target dir exists. |
| `load_skills(data_dir)` | fn | Scan `{data_dir}/skills/*/SKILL.md`, parse frontmatter, check deps, return `Vec<Skill>` |
| `build_skill_prompt(skills)` | fn | Build the system prompt block listing all skills with install status |
| `match_skill_triggers(skills, message)` | fn | Match message against skill triggers, return activated MCP servers (deduped) |

## Skill Directory Format

Skills are stored as directories in `{data_dir}/skills/` with a `SKILL.md` file containing frontmatter (TOML or YAML) between `---` delimiters:

```
~/.omega/skills/
├── google-workspace/
│   └── SKILL.md
└── playwright-mcp-1.0.0/
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
```

YAML format (third-party skills):
```markdown
---
name: playwright-mcp
description: Browser automation via Playwright MCP.
requires: [npx, playwright-mcp]
homepage: https://playwright.dev
---
```

TOML format with trigger + MCP:
```markdown
---
name = "playwright-mcp"
description = "Browser automation via Playwright MCP."
requires = ["npx"]
trigger = "browse|website|click|screenshot|navigate|scrape|web page"

[mcp.playwright]
command = "npx"
args = ["@playwright/mcp", "--headless"]
---
```

YAML format with MCP:
```markdown
---
name: browser-tool
description: Browser automation.
requires: [npx]
trigger: browse|website
mcp-playwright: npx @playwright/mcp --headless
---
```

The YAML parser also extracts `requires` from openclaw-style `metadata` JSON blobs (`"requires":{"bins":[...]}`) when no explicit `requires` field is present.

## Bundled Skills

Core skills are embedded at compile time from `skills/` in the repo root via `include_str!`. On startup, `install_bundled_skills()` writes them to `{data_dir}/skills/{name}/SKILL.md` only if absent, preserving user edits.

| Directory | Skill |
|-----------|-------|
| `skills/claude-code/SKILL.md` | Claude Code CLI (`claude`) |
| `skills/google-workspace/SKILL.md` | Google Workspace CLI (`gog`) |
| `skills/playwright-mcp/SKILL.md` | Playwright MCP browser automation (`npx`) |

## Internal Functions

| Function | Description |
|----------|-------------|
| `parse_skill_file(content)` | Extract frontmatter from `---` delimiters — tries TOML, falls back to YAML |
| `parse_yaml_frontmatter(block)` | Lightweight YAML-style `key: value` parser for frontmatter |
| `parse_yaml_list(val)` | Parse YAML inline list `[a, b, c]` |
| `extract_bins_from_metadata(meta)` | Extract `bins` from openclaw metadata JSON blob |
| `unquote(s)` | Strip surrounding quotes (single or double) |
| `which_exists(tool)` | Check if a CLI tool exists on `$PATH` via `which` |
| `expand_tilde(path)` | Expand `~` to `$HOME` in data_dir paths |

## Dependencies

| Dependency | Usage |
|------------|-------|
| `serde` | Deserialize TOML frontmatter |
| `toml` | Parse TOML (primary frontmatter format) |
| `tracing` | Warn on invalid skill files |
| `omega-core` | `McpServer` type for MCP server declarations |

## Projects

In addition to skills, this crate also handles project loading. Projects are user-defined instruction scopes stored in `~/.omega/projects/`.

### Public API (Projects)

| Item | Kind | Description |
|------|------|-------------|
| `Project` | struct | Loaded project definition (name, instructions, path) |
| `ensure_projects_dir(data_dir)` | fn | Create `{data_dir}/projects/` directory if missing |
| `load_projects(data_dir)` | fn | Scan `{data_dir}/projects/*/ROLE.md`, return `Vec<Project>` sorted by name |
| `get_project_instructions(projects, name)` | fn | Find project by name, return `Option<&str>` of its instructions |

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

- **Project name** = directory name
- **Instructions** = contents of `ROLE.md` (trimmed, must be non-empty)
- Directories without `ROLE.md` or with empty instructions are skipped
- Projects are loaded at startup (restart to pick up new ones)

## Tests

- Valid TOML frontmatter parsing
- Valid YAML frontmatter parsing
- YAML with openclaw metadata JSON extracts bins
- YAML with quoted values (single and double quotes)
- Missing frontmatter returns None
- Empty requires defaults to empty vec
- Empty skill list produces empty prompt
- Prompt format with installed/not-installed status (paths use `*/SKILL.md`)
- `which` detection for known and unknown tools
- Missing skills directory returns empty vec
- Valid skill directory with TOML SKILL.md loads correctly
- Valid skill directory with YAML SKILL.md loads correctly
- Flat skill migration moves `.md` → `{name}/SKILL.md`, skips existing dirs
- Bundled skills deploy to `{name}/SKILL.md`, never overwrite existing files
- TOML frontmatter with trigger and MCP parses correctly
- YAML frontmatter with mcp-* keys parses correctly
- Skill without trigger or MCP has None/empty defaults
- Trigger matching: basic keyword match
- Trigger matching: no match returns empty
- Trigger matching: case-insensitive
- Trigger matching: skips unavailable skills
- Trigger matching: deduplicates by server name
- Trigger matching: no trigger field returns empty
- Trigger matching: no MCP servers returns empty
- Load skills with trigger and MCP from filesystem
- Missing projects directory returns empty vec
- Valid project with ROLE.md loads correctly
- Empty ROLE.md is skipped
- Directory without ROLE.md is skipped
- `get_project_instructions()` returns correct instructions or None
