//! # omega-skills
//!
//! Generic skill and project loader for Omega. Scans `~/.omega/skills/*/SKILL.md`
//! and `~/.omega/projects/*/ROLE.md` for definitions and exposes them to the
//! system prompt so the AI knows what tools and contexts are available.

mod parse;
mod projects;
mod skills;

// Re-export public API — all consumers use `omega_skills::*` paths.
pub use projects::{ensure_projects_dir, get_project_instructions, load_projects, Project};
pub use skills::{
    build_skill_prompt, install_bundled_skills, load_skills, match_skill_triggers,
    migrate_flat_skills, Skill,
};
