# Functionalities: Keywords

## Overview

Keyword matching utilities for build confirmation/cancellation, WhatsApp help intercept, and fact validation. The context-gating keyword arrays (SCHEDULING_KW, RECALL_KW, TASKS_KW, PROJECTS_KW, META_KW, PROFILE_KW, OUTCOMES_KW) were removed -- all prompt sections and context are now always injected.

## Functionalities

| # | Name | Type | Location | Description | Dependencies |
|---|------|------|----------|-------------|--------------|
| 1 | kw_match() | Utility | `backend/src/gateway/keywords.rs` | Matches message text against a keyword array (case-insensitive substring matching) | -- |
| 2 | HELP_KW | Constant | `backend/src/gateway/keywords_data.rs` | Keywords for WhatsApp help intercept | -- |
| 3 | BUILD_CONFIRM_KW | Constant | `backend/src/gateway/keywords_data.rs` | Keywords for build confirmation in 8 languages | -- |
| 4 | BUILD_CANCEL_KW | Constant | `backend/src/gateway/keywords_data.rs` | Keywords for build cancellation in 8 languages | -- |
| 5 | BUILD_CONFIRM_TTL_SECS | Constant | `backend/src/gateway/keywords_data.rs` | TTL for build confirmation prompts | -- |
| 6 | MAX_ACTION_RETRIES | Constant | `backend/src/gateway/keywords_data.rs` | Maximum retry count for action tasks | -- |
| 7 | is_build_confirmed() | Utility | `backend/src/gateway/keywords.rs` | Checks if message confirms a build in 8 languages | -- |
| 8 | is_build_cancelled() | Utility | `backend/src/gateway/keywords.rs` | Checks if message cancels a build in 8 languages | -- |
| 9 | is_valid_fact() | Utility | `backend/src/gateway/keywords.rs` | Validates facts: rejects system keys, too long values, non-personal data | SYSTEM_FACT_KEYS |
| 10 | SYSTEM_FACT_KEYS | Re-export | `backend/src/gateway/keywords.rs` | Re-exported from omega-core config | -- |

## Internal Dependencies

- kw_match() used with HELP_KW for WhatsApp help intercept
- is_build_confirmed()/is_build_cancelled() used by pipeline_builds for build confirmation flow
- is_valid_fact() used by summarizer for fact extraction validation

## Dead Code / Unused

- None detected.
