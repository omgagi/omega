# Technical Specification: omega-skills/Cargo.toml

## File Path

```
crates/omega-skills/Cargo.toml
```

## Purpose

Cargo manifest for the `omega-skills` crate — a generic skill loader that scans `~/.omega/skills/*.md` files with TOML frontmatter and exposes them to the AI via the system prompt.

## Package Metadata

| Field        | Value                                       | Source    |
|--------------|---------------------------------------------|-----------|
| `name`       | `omega-skills`                              | Local     |
| `version`    | `0.1.0`                                     | Workspace |
| `edition`    | `2021`                                      | Workspace |
| `license`    | `MIT OR Apache-2.0`                         | Workspace |
| `repository` | `https://github.com/omega-cortex/omega`     | Workspace |
| `description`| `Skill and plugin system for Omega`         | Local     |

## Dependencies

| Dependency | Resolved Version | Purpose |
|------------|------------------|---------|
| `serde`      | `1` (derive)     | Deserialize TOML frontmatter |
| `toml`       | `0.8`            | Parse TOML frontmatter from skill files |
| `tracing`    | `0.1`            | Warn on invalid skill files |
| `omega-core` | workspace        | `McpServer` type for MCP server declarations |

All dependencies use workspace versions.

## Notes

- Minimal dependency set — only what's needed for file parsing, logging, and MCP types
- No async runtime needed — skill loading is synchronous at startup
- Depends on `omega-core` for the `McpServer` type used in skill MCP server declarations
