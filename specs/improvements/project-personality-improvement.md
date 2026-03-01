# Improvement: Project-Specific OMEGA Personality Greeting

> When a user switches to a project, OMEGA greets them with a persona-branded identity:
> "Hi, I'm *OMEGA Ω Realtor*, what can I do for you?"

## Scope

**Affected:** i18n/format.rs, task_confirmation.rs, process_markers.rs, routing.rs, keywords.rs
**NOT affected:** omega-skills, omega-memory, omega-core, prompt_builder.rs, scheduler_action.rs

## Design Decisions

- **Greeting:** Code-built i18n template (no AI call)
- **Trigger:** `/project` commands + `PROJECT_ACTIVATE:` marker
- **Persona name:** Auto-derived from directory name (kebab-case to Title Case)
- **Customization:** Same template for all projects

## Persona Name Derivation

- `realtor` -> `Realtor`
- `real-estate` -> `Real Estate`
- `tech-youtuber` -> `Tech Youtuber`

## Requirements

| ID | Requirement | Priority |
|----|------------|----------|
| IMP-001 | `humanize_project_name()` utility (kebab to Title Case) | Must |
| IMP-002 | `project_persona_greeting()` i18n (8 languages) | Must |
| IMP-003 | `/project <name>` uses persona greeting | Must |
| IMP-004 | ~~`/project change <name>` uses greeting + monitoring note~~ | ~~Removed~~ |
| IMP-005 | `PROJECT_ACTIVATE:` marker emits persona greeting follow-up | Should |
| IMP-006 | `/setup` completion uses persona greeting | Should |
| IMP-007 | Scheduler action path stays silent (no change) | Must |
| IMP-008 | All 8 languages supported | Must |

## Activation Paths

| Path | Current | Improved |
|------|---------|----------|
| `/project realtor` | "Project 'realtor' activated." | "Hi, I'm *OMEGA Ω Realtor*..." |
| `PROJECT_ACTIVATE:` marker | No greeting (marker stripped) | Follow-up persona greeting |
| `/setup` completion | "configured as your expert..." | Persona greeting |
| Scheduler action | Silent | **No change** |
