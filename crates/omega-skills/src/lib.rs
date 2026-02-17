//! # omega-skills
//!
//! Generic skill loader for Omega. Scans `~/.omega/skills/*/SKILL.md` for skill
//! definitions and exposes them to the system prompt so the AI knows what
//! tools are available.

use serde::Deserialize;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Bundled core skills — embedded at compile time from `skills/` in the repo root.
///
/// Each entry is `(directory_name, content)`. Deployed to `{data_dir}/skills/{name}/SKILL.md`.
const BUNDLED_SKILLS: &[(&str, &str)] = &[(
    "google-workspace",
    include_str!("../../../skills/google-workspace/SKILL.md"),
)];

/// Deploy bundled skills to `{data_dir}/skills/{name}/SKILL.md`, creating
/// subdirectories as needed.
///
/// Never overwrites existing files so user edits are preserved.
pub fn install_bundled_skills(data_dir: &str) {
    let dir = Path::new(&expand_tilde(data_dir)).join("skills");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        warn!("skills: failed to create {}: {e}", dir.display());
        return;
    }
    for (name, content) in BUNDLED_SKILLS {
        let skill_dir = dir.join(name);
        if let Err(e) = std::fs::create_dir_all(&skill_dir) {
            warn!("skills: failed to create {}: {e}", skill_dir.display());
            continue;
        }
        let dest = skill_dir.join("SKILL.md");
        if !dest.exists() {
            if let Err(e) = std::fs::write(&dest, content) {
                warn!("skills: failed to write {}: {e}", dest.display());
            } else {
                info!("skills: installed bundled skill {name}");
            }
        }
    }
}

/// Migrate legacy flat skill files (`{data_dir}/skills/*.md`) to the
/// directory-per-skill layout (`{data_dir}/skills/{name}/SKILL.md`).
///
/// For each `foo.md` found directly in the skills directory, creates a `foo/`
/// subdirectory and moves the file into it as `SKILL.md`. Existing directories
/// are never overwritten.
pub fn migrate_flat_skills(data_dir: &str) {
    let dir = Path::new(&expand_tilde(data_dir)).join("skills");
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut to_migrate: Vec<(PathBuf, String)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("md") {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            if !stem.is_empty() {
                to_migrate.push((path, stem));
            }
        }
    }

    for (file_path, stem) in to_migrate {
        let target_dir = dir.join(&stem);
        if target_dir.exists() {
            // Directory already exists — skip to avoid overwriting.
            continue;
        }
        if let Err(e) = std::fs::create_dir_all(&target_dir) {
            warn!("skills: failed to create {}: {e}", target_dir.display());
            continue;
        }
        let dest = target_dir.join("SKILL.md");
        if let Err(e) = std::fs::rename(&file_path, &dest) {
            warn!(
                "skills: failed to migrate {} → {}: {e}",
                file_path.display(),
                dest.display()
            );
        } else {
            info!(
                "skills: migrated {} → {}",
                file_path.display(),
                dest.display()
            );
        }
    }
}

/// A loaded skill definition.
#[derive(Debug, Clone)]
pub struct Skill {
    /// Short identifier (e.g. "gog").
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// CLI tools this skill depends on.
    pub requires: Vec<String>,
    /// Homepage URL (informational).
    pub homepage: String,
    /// Whether all required CLIs are available on `$PATH`.
    pub available: bool,
    /// Absolute path to the `SKILL.md` file.
    pub path: PathBuf,
}

/// Frontmatter parsed from a `SKILL.md` file (TOML or YAML).
#[derive(Debug, Deserialize)]
struct SkillFrontmatter {
    name: String,
    description: String,
    #[serde(default)]
    requires: Vec<String>,
    #[serde(default)]
    homepage: String,
}

/// Scan `{data_dir}/skills/*/SKILL.md` and return all valid skill definitions.
pub fn load_skills(data_dir: &str) -> Vec<Skill> {
    let dir = Path::new(&expand_tilde(data_dir)).join("skills");
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut skills = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let skill_file = path.join("SKILL.md");
        let content = match std::fs::read_to_string(&skill_file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let Some(fm) = parse_skill_file(&content) else {
            warn!("skills: no valid frontmatter in {}", skill_file.display());
            continue;
        };
        let available = fm.requires.iter().all(|t| which_exists(t));
        skills.push(Skill {
            name: fm.name,
            description: fm.description,
            requires: fm.requires,
            homepage: fm.homepage,
            available,
            path: skill_file,
        });
    }

    skills.sort_by(|a, b| a.name.cmp(&b.name));
    skills
}

/// Build the skill block appended to the system prompt.
///
/// Returns an empty string if there are no skills.
pub fn build_skill_prompt(skills: &[Skill]) -> String {
    if skills.is_empty() {
        return String::new();
    }

    let mut out = String::from(
        "\n\nYou have the following skills available. \
         Before using any skill, you MUST read its file for full instructions. \
         If a tool is not installed, the skill file contains installation \
         instructions — install it first, then use it.\n\nSkills:\n",
    );

    for s in skills {
        let status = if s.available {
            "installed"
        } else {
            "not installed"
        };
        out.push_str(&format!(
            "- {} [{}]: {} → Read {}\n",
            s.name,
            status,
            s.description,
            s.path.display(),
        ));
    }

    out
}

/// Expand `~` to the user's home directory.
fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return format!("{}/{rest}", home.to_string_lossy());
        }
    }
    path.to_string()
}

/// Extract frontmatter delimited by `---` lines.
///
/// Tries TOML first (`key = "value"`), then falls back to YAML-style
/// (`key: value`) so skill files from any source just work.
fn parse_skill_file(content: &str) -> Option<SkillFrontmatter> {
    let trimmed = content.trim_start();
    let rest = trimmed.strip_prefix("---")?;
    let end = rest.find("\n---")?;
    let block = &rest[..end];

    // Try TOML first.
    if let Ok(fm) = toml::from_str::<SkillFrontmatter>(block) {
        return Some(fm);
    }

    // Fallback: parse YAML-style `key: value` lines.
    parse_yaml_frontmatter(block)
}

/// Lightweight YAML-style frontmatter parser.
///
/// Handles flat `key: value` lines and extracts `requires` from either a
/// YAML list (`requires: [a, b]`) or from an openclaw `metadata` JSON
/// blob (`"requires":{"bins":[...]}`). No YAML dependency needed.
fn parse_yaml_frontmatter(block: &str) -> Option<SkillFrontmatter> {
    let mut name = None;
    let mut description = None;
    let mut requires = Vec::new();
    let mut homepage = String::new();
    let mut metadata_line = None;

    for line in block.lines() {
        let line = line.trim();
        if let Some((key, val)) = line.split_once(':') {
            let key = key.trim();
            let val = val.trim();
            match key {
                "name" => name = Some(unquote(val)),
                "description" => description = Some(unquote(val)),
                "homepage" => homepage = unquote(val),
                "requires" => requires = parse_yaml_list(val),
                "metadata" => metadata_line = Some(val.to_string()),
                _ => {}
            }
        }
    }

    // If no explicit `requires`, try extracting from openclaw metadata JSON.
    if requires.is_empty() {
        if let Some(meta) = &metadata_line {
            requires = extract_bins_from_metadata(meta);
        }
    }

    Some(SkillFrontmatter {
        name: name?,
        description: description?,
        requires,
        homepage,
    })
}

/// Parse a YAML-style inline list: `[a, b, c]` or `["a", "b"]`.
fn parse_yaml_list(val: &str) -> Vec<String> {
    let trimmed = val.trim();
    let inner = trimmed
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or("");
    if inner.is_empty() {
        return Vec::new();
    }
    inner
        .split(',')
        .map(|s| unquote(s.trim()))
        .filter(|s| !s.is_empty())
        .collect()
}

/// Extract `bins` from an openclaw metadata JSON blob.
///
/// Looks for `"requires":{"bins":["tool1","tool2"]}` without a full JSON parser.
fn extract_bins_from_metadata(meta: &str) -> Vec<String> {
    let Some(idx) = meta.find("\"bins\"") else {
        return Vec::new();
    };
    let rest = &meta[idx..];
    let Some(start) = rest.find('[') else {
        return Vec::new();
    };
    let Some(end) = rest[start..].find(']') else {
        return Vec::new();
    };
    let inner = &rest[start + 1..start + end];
    inner
        .split(',')
        .map(|s| unquote(s.trim()))
        .filter(|s| !s.is_empty())
        .collect()
}

/// Strip surrounding quotes (single or double) from a string.
fn unquote(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

// ---------------------------------------------------------------------------
// Projects
// ---------------------------------------------------------------------------

/// A loaded project definition.
#[derive(Debug, Clone)]
pub struct Project {
    /// Directory name (e.g. "real-estate").
    pub name: String,
    /// Contents of `INSTRUCTIONS.md`.
    pub instructions: String,
    /// Absolute path to the project directory.
    pub path: PathBuf,
}

/// Create `{data_dir}/projects/` if it doesn't exist.
pub fn ensure_projects_dir(data_dir: &str) {
    let dir = Path::new(&expand_tilde(data_dir)).join("projects");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        warn!("projects: failed to create {}: {e}", dir.display());
    }
}

/// Scan `{data_dir}/projects/*/INSTRUCTIONS.md` and return all valid projects.
pub fn load_projects(data_dir: &str) -> Vec<Project> {
    let dir = Path::new(&expand_tilde(data_dir)).join("projects");
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut projects = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let instructions_path = path.join("INSTRUCTIONS.md");
        let content = match std::fs::read_to_string(&instructions_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let trimmed = content.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            continue;
        }
        projects.push(Project {
            name,
            instructions: trimmed,
            path,
        });
    }

    projects.sort_by(|a, b| a.name.cmp(&b.name));
    projects
}

/// Find a project by name and return its instructions.
pub fn get_project_instructions<'a>(projects: &'a [Project], name: &str) -> Option<&'a str> {
    projects
        .iter()
        .find(|p| p.name == name)
        .map(|p| p.instructions.as_str())
}

/// Check whether a CLI tool exists on `$PATH`.
fn which_exists(tool: &str) -> bool {
    std::process::Command::new("which")
        .arg(tool)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_frontmatter() {
        let content = "\
---
name = \"gog\"
description = \"Google Workspace CLI.\"
requires = [\"gog\"]
homepage = \"https://gogcli.sh\"
---

Some body text.
";
        let fm = parse_skill_file(content).unwrap();
        assert_eq!(fm.name, "gog");
        assert_eq!(fm.description, "Google Workspace CLI.");
        assert_eq!(fm.requires, vec!["gog"]);
        assert_eq!(fm.homepage, "https://gogcli.sh");
    }

    #[test]
    fn test_parse_yaml_frontmatter() {
        let content = "\
---
name: playwright-mcp
description: Browser automation via Playwright MCP.
requires: [npx, playwright-mcp]
homepage: https://playwright.dev
---

Some body text.
";
        let fm = parse_skill_file(content).unwrap();
        assert_eq!(fm.name, "playwright-mcp");
        assert_eq!(fm.description, "Browser automation via Playwright MCP.");
        assert_eq!(fm.requires, vec!["npx", "playwright-mcp"]);
        assert_eq!(fm.homepage, "https://playwright.dev");
    }

    #[test]
    fn test_parse_yaml_openclaw_metadata() {
        let content = "\
---
name: playwright-mcp
description: Browser automation.
metadata: {\"openclaw\":{\"requires\":{\"bins\":[\"playwright-mcp\",\"npx\"]}}}
---
";
        let fm = parse_skill_file(content).unwrap();
        assert_eq!(fm.name, "playwright-mcp");
        assert_eq!(fm.requires, vec!["playwright-mcp", "npx"]);
    }

    #[test]
    fn test_parse_yaml_quoted_values() {
        let content = "\
---
name: \"my-tool\"
description: 'A quoted description.'
---
";
        let fm = parse_skill_file(content).unwrap();
        assert_eq!(fm.name, "my-tool");
        assert_eq!(fm.description, "A quoted description.");
    }

    #[test]
    fn test_parse_no_frontmatter() {
        assert!(parse_skill_file("Just plain text.").is_none());
    }

    #[test]
    fn test_parse_empty_requires() {
        let content = "\
---
name = \"simple\"
description = \"No deps.\"
---
";
        let fm = parse_skill_file(content).unwrap();
        assert!(fm.requires.is_empty());
    }

    #[test]
    fn test_build_skill_prompt_empty() {
        assert!(build_skill_prompt(&[]).is_empty());
    }

    #[test]
    fn test_build_skill_prompt_formats_correctly() {
        let skills = vec![
            Skill {
                name: "gog".into(),
                description: "Google Workspace CLI.".into(),
                requires: vec!["gog".into()],
                homepage: "https://gogcli.sh".into(),
                available: true,
                path: PathBuf::from("/home/user/.omega/skills/gog/SKILL.md"),
            },
            Skill {
                name: "missing".into(),
                description: "Not installed tool.".into(),
                requires: vec!["nope".into()],
                homepage: String::new(),
                available: false,
                path: PathBuf::from("/home/user/.omega/skills/missing/SKILL.md"),
            },
        ];
        let prompt = build_skill_prompt(&skills);
        assert!(prompt.contains("gog [installed]"));
        assert!(prompt.contains("missing [not installed]"));
        assert!(prompt.contains("Read /home/user/.omega/skills/gog/SKILL.md"));
    }

    #[test]
    fn test_which_exists_known_tool() {
        // `ls` should exist on any Unix system.
        assert!(which_exists("ls"));
    }

    #[test]
    fn test_which_exists_missing_tool() {
        assert!(!which_exists("__omega_nonexistent_tool_42__"));
    }

    #[test]
    fn test_load_skills_missing_dir() {
        let skills = load_skills("/tmp/__omega_test_no_such_dir__");
        assert!(skills.is_empty());
    }

    #[test]
    fn test_load_skills_valid() {
        let tmp = std::env::temp_dir().join("__omega_test_skills_valid__");
        let _ = std::fs::remove_dir_all(&tmp);
        let skill_dir = tmp.join("skills/my-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname = \"my-skill\"\ndescription = \"A test skill.\"\n---\n\nBody.",
        )
        .unwrap();

        let skills = load_skills(tmp.to_str().unwrap());
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "my-skill");
        assert_eq!(skills[0].description, "A test skill.");
        assert!(skills[0].path.ends_with("my-skill/SKILL.md"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_load_skills_yaml_format() {
        let tmp = std::env::temp_dir().join("__omega_test_skills_yaml__");
        let _ = std::fs::remove_dir_all(&tmp);
        let skill_dir = tmp.join("skills/playwright");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: playwright\ndescription: Browser automation.\nrequires: [npx]\n---\n\nBody.",
        )
        .unwrap();

        let skills = load_skills(tmp.to_str().unwrap());
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "playwright");
        assert_eq!(skills[0].description, "Browser automation.");
        assert_eq!(skills[0].requires, vec!["npx"]);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_migrate_flat_skills() {
        let tmp = std::env::temp_dir().join("__omega_test_migrate__");
        let _ = std::fs::remove_dir_all(&tmp);
        let skills_dir = tmp.join("skills");
        std::fs::create_dir_all(&skills_dir).unwrap();

        // Create a flat skill file.
        std::fs::write(
            skills_dir.join("my-tool.md"),
            "---\nname = \"my-tool\"\ndescription = \"Test.\"\n---\n",
        )
        .unwrap();

        // Create a directory that already exists (should not be touched).
        let existing_dir = skills_dir.join("existing");
        std::fs::create_dir_all(&existing_dir).unwrap();
        std::fs::write(existing_dir.join("SKILL.md"), "original").unwrap();
        // Also create a flat file with the same stem — should be skipped.
        std::fs::write(skills_dir.join("existing.md"), "flat version").unwrap();

        migrate_flat_skills(tmp.to_str().unwrap());

        // my-tool.md should have been moved to my-tool/SKILL.md.
        assert!(!skills_dir.join("my-tool.md").exists());
        assert!(skills_dir.join("my-tool/SKILL.md").exists());
        let content = std::fs::read_to_string(skills_dir.join("my-tool/SKILL.md")).unwrap();
        assert!(content.contains("my-tool"));

        // existing/ should be untouched.
        let existing_content = std::fs::read_to_string(existing_dir.join("SKILL.md")).unwrap();
        assert_eq!(existing_content, "original");
        // The flat existing.md should still be there (skipped because dir exists).
        assert!(skills_dir.join("existing.md").exists());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_load_projects_missing_dir() {
        let projects = load_projects("/tmp/__omega_test_no_such_projects_dir__");
        assert!(projects.is_empty());
    }

    #[test]
    fn test_load_projects_valid() {
        let tmp = std::env::temp_dir().join("__omega_test_projects_valid__");
        let _ = std::fs::remove_dir_all(&tmp);
        let proj_dir = tmp.join("projects/my-project");
        std::fs::create_dir_all(&proj_dir).unwrap();
        std::fs::write(
            proj_dir.join("INSTRUCTIONS.md"),
            "You are a helpful assistant.",
        )
        .unwrap();

        let projects = load_projects(tmp.to_str().unwrap());
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "my-project");
        assert_eq!(projects[0].instructions, "You are a helpful assistant.");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_load_projects_empty_instructions() {
        let tmp = std::env::temp_dir().join("__omega_test_projects_empty__");
        let _ = std::fs::remove_dir_all(&tmp);
        let proj_dir = tmp.join("projects/empty-proj");
        std::fs::create_dir_all(&proj_dir).unwrap();
        std::fs::write(proj_dir.join("INSTRUCTIONS.md"), "   \n  ").unwrap();

        let projects = load_projects(tmp.to_str().unwrap());
        assert!(projects.is_empty(), "empty instructions should be skipped");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_load_projects_no_instructions_file() {
        let tmp = std::env::temp_dir().join("__omega_test_projects_no_file__");
        let _ = std::fs::remove_dir_all(&tmp);
        let proj_dir = tmp.join("projects/no-file");
        std::fs::create_dir_all(&proj_dir).unwrap();
        // No INSTRUCTIONS.md created.

        let projects = load_projects(tmp.to_str().unwrap());
        assert!(
            projects.is_empty(),
            "dir without INSTRUCTIONS.md should be skipped"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_get_project_instructions() {
        let projects = vec![Project {
            name: "stocks".into(),
            instructions: "Track my portfolio.".into(),
            path: PathBuf::from("/home/user/.omega/projects/stocks"),
        }];
        assert_eq!(
            get_project_instructions(&projects, "stocks"),
            Some("Track my portfolio.")
        );
        assert!(get_project_instructions(&projects, "unknown").is_none());
    }

    #[test]
    fn test_install_bundled_skills_creates_files() {
        let tmp = std::env::temp_dir().join("__omega_test_bundled__");
        let _ = std::fs::remove_dir_all(&tmp);
        install_bundled_skills(tmp.to_str().unwrap());
        let dest = tmp.join("skills/google-workspace/SKILL.md");
        assert!(dest.exists(), "bundled skill should be deployed");
        let content = std::fs::read_to_string(&dest).unwrap();
        assert!(content.contains("google-workspace"));
        // Run again — should not overwrite.
        std::fs::write(&dest, "custom").unwrap();
        install_bundled_skills(tmp.to_str().unwrap());
        let after = std::fs::read_to_string(&dest).unwrap();
        assert_eq!(after, "custom", "should not overwrite user edits");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
