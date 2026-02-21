# Omega Self-Improvement: Autonomous Skill Learning

## What Is Skill Improvement?

Skill improvement is Omega's ability to learn from its own mistakes. When Omega makes an error while using a skill (e.g., stops a search too early, uses a wrong API parameter, misses an edge case), it fixes the problem immediately, then updates the skill's instructions so the same mistake never happens again.

## How It Works

### Detection

The AI detects skill mistakes during normal operation:
1. **Search returned no results** when results were expected — retry with a different approach
2. **API call failed** — investigate and adapt the strategy
3. **Output doesn't match expectations** — try alternative methods before giving up

### The SKILL_IMPROVE Marker

Format: `SKILL_IMPROVE: <skill-name> | <lesson learned>`

Example: `SKILL_IMPROVE: google-workspace | Always search contacts by both name and email address, not just name`

### Processing Pipeline

When a SKILL_IMPROVE marker is found:

1. Parse the skill name and lesson
2. Locate `{data_dir}/skills/{skill_name}/SKILL.md`
3. Read the file
4. If a `## Lessons Learned` section exists, append the lesson under it
5. If no such section exists, create it at the end of the file
6. Write the updated file back
7. Push a `SkillImproved` or `SkillImproveFailed` result for user confirmation

### Confirmation

The gateway sends a localized confirmation message after processing:
- Success: `✓ Skill updated: google-workspace — Always search by both name and email`
- Failure: `✗ Failed to update skill google-workspace: skill not found`

## Design Decisions

### Why Append to SKILL.md?

The skill's instruction file is what gets injected into the AI's context. By appending lessons directly to the file, future invocations of the skill automatically benefit from past mistakes. No external tracking needed.

### Why a Dedicated Section?

The `## Lessons Learned` section keeps lessons organized and separated from the skill's core instructions. It's easy to review, edit, or prune manually.

### Why Gateway-Managed?

The gateway handles the file I/O, not the AI. This ensures atomic writes, proper error handling, and consistent confirmation messages. The AI only needs to emit the marker.

## The Behavior Loop

```
AI uses a skill → something goes wrong
    │
    ▼
AI detects the error (no results, unexpected output, API failure)
    │
    ▼
AI retries with a different approach → succeeds
    │
    ▼
AI emits SKILL_IMPROVE: skill-name | lesson learned
    │
    ▼
Gateway appends lesson to skill's SKILL.md under ## Lessons Learned
    │
    ▼
AI apologizes briefly, confirms the task is done
    │
    ▼
Next time the skill is used, the lesson is in the AI's context → mistake never repeats
```

## Examples

### Example 1: Google Contacts Search

```
User: Find John's phone number in my contacts
Omega: [searches by name only, finds nothing]
Omega: [detects mistake — should have searched by email too]
Omega: [retries with broader search, finds John]
Omega: Here's John's number: +1-555-0123
SKILL_IMPROVE: google-workspace | Always search contacts by both name and email address — name-only search may miss contacts stored with display names different from the search query
```

### Example 2: Web Scraping

```
User: Get the price of AAPL from Yahoo Finance
Omega: [first attempt hits a captcha page]
Omega: [detects failure, tries alternative selector]
Omega: AAPL is at $187.50
SKILL_IMPROVE: playwright-mcp | Yahoo Finance may serve captcha pages — always check page title before extracting data, and retry with a fresh browser context if captcha detected
```

## Self-Audit

Beyond skill improvement, OMEGA monitors its own behavior for anomalies. The self-audit instruction in the system prompt tells OMEGA to flag immediately when:

- Output doesn't match expectations
- Claims can't be backed up with evidence
- Tools fail silently
- Results don't add up

OMEGA has read access to its own audit trail at `~/.omega/memory.db`:
- `audit_log` — every exchange with model used, processing time, status
- `conversations` — conversation history
- `facts` — user profile data

When something doesn't add up, OMEGA can query these tables to verify its own behavior before reporting.
