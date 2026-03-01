# Technical Specification: `backend/crates/omega-skills/src/lib.rs`

## File

| Field | Value |
|-------|-------|
| **Path** | `backend/crates/omega-skills/src/lib.rs` (re-export orchestrator) |
| **Crate** | `omega-skills` |
| **Role** | Generic skill and project loader — scans `~/.omega/skills/*/SKILL.md` and `~/.omega/projects/*/ROLE.md`, exposes them to the system prompt |

## Module Structure

The crate is split into 3 focused submodules; `lib.rs` is a thin re-export orchestrator:

| File | Responsibility |
|------|---------------|
| `lib.rs` | `mod` declarations + `pub use` re-exports — all consumers use `omega_skills::*` paths |
| `skills.rs` | Skill loading, parsing, bundled install, migration, trigger matching, MCP activation, MCP command validation |
| `projects.rs` | Project loading, frontmatter parsing, `ensure_projects_dir`, `get_project_instructions` |
| `parse.rs` | Shared `pub(crate)` parsing utilities: `expand_tilde`, `unquote`, `parse_yaml_list`, `extract_bins_from_metadata`, `which_exists`, `data_path` |

## Purpose

Loads skill definitions from `SKILL.md` files inside per-skill directories. Frontmatter can be TOML (`key = "value"`) or YAML (`key: value`) — the parser tries TOML first, then falls back to YAML-style parsing so skill files from any source (npm packages, third parties) just work. Each skill file declares a name, description, required CLI tools, and optional homepage. The loader checks whether required tools are installed and builds a prompt block that tells the AI what skills exist and where to read full instructions.

Also loads project definitions from `ROLE.md` files. Projects support optional TOML/YAML frontmatter (between `---` delimiters) for declaring skill dependencies. Files without frontmatter are loaded as plain instruction text for backward compatibility.

## Public API

| Item | Kind | Description |
|------|------|-------------|
| `Skill` | struct | Loaded skill definition (name, description, requires, homepage, available, path, trigger, mcp_servers) |
| `Project` | struct | Loaded project definition (name, instructions, path, skills) |
| `install_bundled_skills(data_dir)` | fn | Deploy bundled core skills to `{data_dir}/skills/{name}/SKILL.md`, creating subdirs if needed. Never overwrites existing files. |
| `migrate_flat_skills(data_dir)` | fn | Auto-migrate legacy flat `.md` files to `{name}/SKILL.md` directory layout. Skips if target dir exists. |
| `load_skills(data_dir)` | fn | Scan `{data_dir}/skills/*/SKILL.md`, parse frontmatter, check deps, return `Vec<Skill>`. Includes path traversal protection. |
| `build_skill_prompt(skills)` | fn | Build the system prompt block listing all skills with install status |
| `match_skill_triggers(skills, message)` | fn | Match message against skill triggers, return activated MCP servers (deduped) |
| `ensure_projects_dir(data_dir)` | fn | Create `{data_dir}/projects/` directory if missing |
| `load_projects(data_dir)` | fn | Scan `{data_dir}/projects/*/ROLE.md`, parse optional frontmatter, return `Vec<Project>` sorted by name. Includes path traversal protection. |
| `get_project_instructions(projects, name)` | fn | Find project by name, return `Option<&str>` of its instructions |

## Structs

### `Skill`

```rust
pub struct Skill {
    pub name: String,           // Short identifier (e.g. "gog")
    pub description: String,    // Human-readable description
    pub requires: Vec<String>,  // CLI tools this skill depends on
    pub homepage: String,       // Homepage URL (informational)
    pub available: bool,        // Whether all required CLIs are on $PATH
    pub path: PathBuf,          // Absolute path to the SKILL.md file
    pub trigger: Option<String>,// Pipe-separated trigger keywords
    pub mcp_servers: Vec<McpServer>, // MCP servers declared by this skill
}
```

### `Project`

```rust
pub struct Project {
    pub name: String,           // Directory name (e.g. "real-estate")
    pub instructions: String,   // Contents of ROLE.md (body after frontmatter)
    pub path: PathBuf,          // Absolute path to the project directory
    pub skills: Vec<String>,    // Skills declared in ROLE.md frontmatter
}
```

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
| `skills/skill-creator/SKILL.md` | Skill creator (meta-skill for creating new skills) |
| `skills/ibkr-trader/SKILL.md` | Interactive Brokers trading via omega-trader binary |

## Security

### Path Traversal Protection

Both `load_skills()` and `load_projects()` include path traversal guards. After reading a directory entry, the path is canonicalized via `std::fs::canonicalize()` and verified to still be under the expected parent directory using `Path::starts_with()` (component-aware, not string prefix). Entries that escape the parent directory are logged with a warning and skipped.

### MCP Command Validation

The `is_safe_mcp_command()` function validates MCP command names before they are accepted. It allows only alphanumeric characters, hyphens, underscores, dots, forward slashes (for paths like `/usr/bin/foo`), and `@` (for scoped packages like `@playwright/mcp`). Shell metacharacters (`;`, `|`, `&`, `$`, `` ` ``, `>`, `<`, `(`, `)`, `{`, `}`, `!`, `~`, `#`, spaces) are rejected. Empty commands are also rejected.

This validation is applied both during TOML frontmatter loading (in `load_skills()`) and during YAML frontmatter parsing (in `parse_yaml_frontmatter()`). Rejected commands are logged with a warning and the corresponding MCP server entry is dropped.

## Internal Functions

### `skills.rs`

| Function | Description |
|----------|-------------|
| `parse_skill_file(content)` | Extract frontmatter from `---` delimiters — tries TOML, falls back to YAML |
| `parse_yaml_frontmatter(block)` | Lightweight YAML-style `key: value` parser for frontmatter |
| `is_safe_mcp_command(command)` | Validate MCP command contains only safe characters (no shell metacharacters) |

### `projects.rs`

| Function | Description |
|----------|-------------|
| `parse_project_frontmatter(content)` | Parse optional `---` delimited frontmatter from ROLE.md — tries TOML, YAML fallback. Returns (frontmatter, body). Files without `---` return default frontmatter and full content. |

### `parse.rs` (shared utilities)

| Function | Description |
|----------|-------------|
| `expand_tilde(path)` | Expand `~` to `$HOME` in data_dir paths |
| `unquote(s)` | Strip surrounding quotes (single or double) |
| `parse_yaml_list(val)` | Parse YAML inline list `[a, b, c]` |
| `extract_bins_from_metadata(meta)` | Extract `bins` from openclaw metadata JSON blob |
| `which_exists(tool)` | Check if a CLI tool exists on `$PATH` using a pure-Rust PATH search (iterates `$PATH` directories, checks for file existence). No subprocess spawned. |
| `data_path(data_dir, sub)` | Resolve `{data_dir}/{sub}` with tilde expansion |

## Dependencies

| Dependency | Usage |
|------------|-------|
| `serde` | Deserialize TOML/YAML frontmatter for skills and projects |
| `toml` | Parse TOML (primary frontmatter format) |
| `tracing` | Warn on invalid skill files, path traversal blocks, unsafe MCP commands |
| `omega-core` | `McpServer` type for MCP server declarations |

## Projects

In addition to skills, this crate also handles project loading. Projects are user-defined instruction scopes stored in `~/.omega/projects/`.

### Project Frontmatter

ROLE.md files support optional frontmatter between `---` delimiters. The frontmatter is parsed as TOML first, with YAML fallback. Currently the only frontmatter field is `skills` (a list of skill names the project depends on).

TOML format:
```markdown
---
skills = ["ibkr-trader", "playwright-mcp"]
---

You are a trading assistant.
```

YAML format:
```markdown
---
skills: [ibkr-trader, playwright-mcp]
---

You are a trading assistant.
```

Files without `---` delimiters are loaded with empty skills list and the full content as instructions (backward compatible).

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
- **Instructions** = body of `ROLE.md` after frontmatter (trimmed, must be non-empty)
- **Skills** = optional list from frontmatter, defaults to empty
- Directories without `ROLE.md` or with empty instructions are skipped
- Projects are loaded at startup (restart to pick up new ones)
- Path traversal protection: canonicalized paths verified to be under `{data_dir}/projects/`

## Tests

### Skills tests (skills.rs)

- Valid TOML frontmatter parsing
- Valid YAML frontmatter parsing
- YAML with openclaw metadata JSON extracts bins
- YAML with quoted values (single and double quotes)
- Missing frontmatter returns None
- Empty requires defaults to empty vec
- Empty skill list produces empty prompt
- Prompt format with installed/not-installed status (paths use `*/SKILL.md`)
- `which_exists` detection for known (`ls`) and unknown tools (pure-Rust PATH search)
- Missing skills directory returns empty vec
- Valid skill directory with TOML SKILL.md loads correctly
- Valid skill directory with YAML SKILL.md loads correctly
- Flat skill migration moves `.md` to `{name}/SKILL.md`, skips existing dirs
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
- `is_safe_mcp_command`: accepts valid commands (alphanumeric, hyphens, underscores, paths, `@` scopes)
- `is_safe_mcp_command`: rejects empty, semicolons, pipes, ampersands, `$()`, backticks, redirects, parens
- Malicious MCP command rejected during TOML parsing flow
- Malicious MCP command rejected during YAML parsing flow

### Project tests (projects.rs)

- Missing projects directory returns empty vec
- Valid project with ROLE.md loads correctly
- Empty ROLE.md is skipped
- Directory without ROLE.md is skipped
- `get_project_instructions()` returns correct instructions or None
- Project frontmatter with TOML `skills` list parsed correctly
- Project frontmatter with YAML `skills` list parsed correctly
- Project without frontmatter: default empty skills, full content as body
- Project with empty skills list: parsed correctly
- Load project with frontmatter from filesystem: skills populated, frontmatter stripped from instructions
- Load project without frontmatter: backward compatible, full content as instructions
