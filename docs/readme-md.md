# README.md Documentation

## Overview

The README.md is the **primary entry point and front door of the Omega project**. It serves as the first document users encounter on GitHub. Structured for progressive disclosure: hook, value proposition, capabilities, architecture, usage, installation.

---

## Structure and Communication Strategy

### Title + One-Line Install (First 5 Seconds)
- **Tagline:** "Your AI, your server, your rules." — ownership, control, agency
- **One-line install** appears immediately after the description, before any explanation
- **Goal:** Convert casual visitors into active users within seconds

### Why Omega (Value Proposition)
Three pillars replace a bullet list:
1. **Autonomous, not assistive** — differentiates from chatbots
2. **Powered by Claude Code** — leverages Anthropic's own tooling, no API keys
3. **Runs on your machine** — privacy guarantee, no cloud dependency

Each pillar is a paragraph, not a bullet. This forces the reader to engage with the argument rather than scan past it.

### What It Does (Capability Showcase)
11 sections, each with a heading and 2-4 sentences. Ordered by impact:
1. Smart Model Routing (automatic, invisible)
2. Real Memory (persistent, semantic)
3. Reward-Based Learning (self-improving)
4. Task Scheduling (autonomous execution)
5. Multi-Agent Build Pipeline (software development)
6. Heartbeat Monitoring (proactive)
7. Skill System (extensible)
8. Project System (domain isolation)
9. OS-Level Sandbox (security)
10. Multi-Language (8 languages)
11. Multi-Channel (Telegram + WhatsApp)

### Architecture (Technical Confidence)
- ASCII diagram shows the full message flow
- 6-crate table maps responsibilities

### How It Works (Pipeline Transparency)
- 12-stage numbered pipeline (previously 10)
- Token efficiency table quantifies savings
- Background loops section covers autonomous behavior
- Marker protocol table shows AI-to-gateway communication

### Commands, API, Installation, Configuration
Standard reference sections. Commands table includes all 20 bot commands. API documents 3 endpoints. Installation offers 3 paths (curl, source, service). Configuration shows minimal working example.

### Codebase Stats
Quantitative summary: 154 functionalities, 6 crates, 20 commands, 21 markers, 6 providers, 2 channels, 8 languages, 13 migrations, 3 security layers.

---

## Audience Segmentation

| Audience | Primary Sections | Time to Decision |
|----------|-----------------|------------------|
| New user | Title + Why + Install | 30 seconds |
| Developer | Architecture + How It Works + Codebase | 2 minutes |
| Decision-maker | Why + What It Does + Codebase Stats | 90 seconds |
| Contributor | Architecture + Commands + Configuration | 3 minutes |

---

## Key Design Decisions

1. **Install before explanation** — the curl command appears in the first 5 lines, before any feature description
2. **"Why" before "What"** — motivation precedes capability list
3. **No provider table** — Claude Code is the default and only recommended provider; others mentioned in crate description only
4. **Quantitative claims** — token savings percentages, exact counts, stage numbers
5. **No trading section** — omega-trader is mentioned only in the skill system context, keeping focus on the core agent
6. **No development section** — build commands are in CLAUDE.md and docs; README stays user-focused
