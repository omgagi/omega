# Audit Fix Progress

## Summary
- **Audit report:** docs/audits/audit-full-2026-03-01.md
- **Total findings:** 109
- **P0 (Critical):** 4
- **P1 (Major):** 15
- **P2 (Minor):** 30
- **P3 (Suggestions):** 12 SKIPPED
- **Specs/Docs Drift:** 48 (out of scope for auto-fix)

## Priority Pass Status

| Priority | Status | Findings | Fixed | Escalated | Commit |
|----------|--------|----------|-------|-----------|--------|
| P0 | COMPLETE | 4 | 4 | 0 | `8c97586` |
| P1 | COMPLETE | 15 | 14 | 1 | `0b6d08e` |
| P2 | COMPLETE | 30 | 30 | 0 | `dfebb30`, `b4c88c7` |
| P3 | SKIPPED | 12 | 0 | 0 | -- |

## Findings Detail

### P0: Critical
| ID | Title | Status | Fix Commit |
|----|-------|--------|------------|
| P0-001 | Auth bypass -- empty allowed_users permits all users | FIXED | `8c97586` |
| P0-002 | Sandbox bypass via relative paths | FIXED | `8c97586` |
| P0-003 | ToolExecutor.config_path never set | FIXED | `8c97586` |
| P0-004 | Seatbelt doesn't block config.toml writes | FIXED | `8c97586` |

### P1: Major
| ID | Title | Status | Fix Commit |
|----|-------|--------|------------|
| P1-001 | API server no auth by default | FIXED | `0b6d08e` |
| P1-002 | constant_time_eq leaks token length | FIXED | `0b6d08e` |
| P1-003 | Case-sensitive role tag matching | FIXED | `0b6d08e` |
| P1-004 | Agent name path traversal unvalidated | FIXED | `0b6d08e` |
| P1-005 | String-level prefix matching false positives | FIXED | `0b6d08e` |
| P1-006 | Per-message filesystem I/O for project loading | FIXED | `0b6d08e` |
| P1-007 | O(n^2) message cloning in agentic loops | N/A | -- (claude-code only, HTTP providers unused) |
| P1-008 | Missing tests for auth module | FIXED | `0b6d08e` |
| P1-009 | Landlock restrictions skip non-existent paths | FIXED | `0b6d08e` |
| P1-010 | selfcheck.rs no timeout on HTTP request | FIXED | `0b6d08e` |
| P1-011 | Repository URL inconsistency | FIXED | `0b6d08e` |
| P1-012 | Blocking I/O in async runtime (which) | FIXED | `0b6d08e` |
| P1-013 | Telegram send_text swallows errors | FIXED | `0b6d08e` |
| P1-014 | WhatsApp Mutex held during retry_send | FIXED | `0b6d08e` |
| P1-015 | WhatsApp SQLite missing WAL mode | FIXED | `0b6d08e` |

### P2: Minor
| ID | Title | Status | Fix Commit |
|----|-------|--------|------------|
| P2-SEC-001 | Override phrase detection bypassable | FIXED | `b4c88c7` |
| P2-SEC-002 | No path traversal validation in skill loading | FIXED | `dfebb30` |
| P2-SEC-003 | MCP command field trusted without validation | FIXED | `b4c88c7` |
| P2-PERF-001 | WhatsApp store INSERTs without transaction | FIXED | `dfebb30` |
| P2-PERF-002 | WhatsApp store INSERTs without transaction (skdm) | FIXED | `dfebb30` |
| P2-PERF-003 | New reqwest::Client per voice message | FIXED | `dfebb30` |
| P2-PERF-004 | DESC + reverse instead of ASC subquery | FIXED | `b4c88c7` |
| P2-PERF-005 | 7 sequential DB queries could be parallelized | FIXED | `b4c88c7` |
| P2-DEBT-001 | migrate_layout blocking I/O | FIXED | `b4c88c7` |
| P2-DEBT-002 | install_bundled_prompts blocking I/O | FIXED | `b4c88c7` |
| P2-DEBT-003 | config::load blocking I/O | FIXED | `b4c88c7` |
| P2-DEBT-004 | send_photo_bytes swallows errors | FIXED | `0b6d08e` |
| P2-DEBT-005 | split_message duplicated | FIXED | `dfebb30` |
| P2-DEBT-006 | download_telegram_file no size limit | FIXED | `dfebb30` |
| P2-DEBT-007 | format! for SQL offset | FIXED | `b4c88c7` |
| P2-DEBT-008 | get_due_tasks returns 8-element tuple | FIXED | `b4c88c7` |
| P2-DEBT-009 | build_system_prompt has 9 parameters | FIXED | `b4c88c7` |
| P2-DEBT-010 | Gateway::new takes 14 parameters | FIXED | `b4c88c7` |
| P2-DEBT-011 | println in non-interactive init | FIXED | `b4c88c7` |
| P2-DEBT-012 | TODO comments in builds modules | FIXED | `b4c88c7` |
| P2-DEAD-001 | Dead code in routing.rs | FIXED | `dfebb30` |
| P2-DEAD-002 | #[allow(dead_code)] on unused fields | FIXED | `b4c88c7` |
| P2-TEST-001 | No tests for OmegaError | FIXED | `dfebb30` |
| P2-TEST-002 | No tests for message types | FIXED | `dfebb30` |
| P2-TEST-003 | Provider factory has no tests | FIXED | `b4c88c7` |
| P2-TEST-004 | Pipeline has no direct tests | FIXED | `b4c88c7` |
| P2-TEST-005 | Marker processing orchestration untested | FIXED | `b4c88c7` |
| P2-TEST-006 | AuditLogger::log() has no test | FIXED | `dfebb30` |
| P2-COMP-001 | Stale .gitignore entry | FIXED | `dfebb30` |
| P2-COMP-002 | unsafe not in CLAUDE.md exemptions | FIXED | `dfebb30` |

### P3: Suggestions
| ID | Title | Status |
|----|-------|--------|
| P3-001 | Wrap data in Arc | SKIPPED |
| P3-002 | Add spec files for undocumented modules | SKIPPED |
| P3-003 | Update gateway spec file count | SKIPPED |
| P3-004 | Update config spec for WhatsApp fields | SKIPPED |
| P3-005 | Replace which subprocess with pure-Rust | SKIPPED |
| P3-006 | match_skill_triggers clones | SKIPPED |
| P3-007 | expand_tilde bare ~ handling | SKIPPED |
| P3-008 | README license mismatch | SKIPPED |
| P3-009 | Hardcoded 120s timeout | SKIPPED |
| P3-010 | Double JSON parsing in claude_code | SKIPPED |
| P3-011 | WhatsApp store zero test coverage | SKIPPED |
| P3-012 | MCP client no integration test | SKIPPED |
