---
name = "playwright-mcp"
description = "Browser automation via Playwright MCP."
requires = ["npx"]
trigger = "browse|website|click|screenshot|navigate|scrape|web page"

[mcp.playwright]
command = "npx"
args = ["@playwright/mcp", "--headless"]
---

# Playwright MCP

Browser automation via the Playwright MCP server. Activated automatically when your message mentions browsing, websites, clicking, screenshots, navigation, or scraping.

## What It Does

When triggered, Omega configures a Playwright MCP server for Claude Code, giving it browser automation tools:

- **Navigate** to URLs and interact with web pages
- **Click** elements, fill forms, and submit data
- **Screenshot** pages for visual inspection
- **Scrape** content from websites
- **Wait** for elements to load and pages to render

## Requirements

- `npx` (comes with Node.js)
- No global install needed — `npx` fetches `@playwright/mcp` on demand

## How It Works

1. User message matches a trigger keyword (e.g. "browse google.com")
2. Omega writes a temporary `.claude/settings.local.json` with the Playwright MCP server config
3. Claude Code picks up the MCP server and gains browser tools
4. After the response, the temporary config is cleaned up

## Example Messages

- "Browse example.com and summarize the content"
- "Take a screenshot of my website"
- "Navigate to the login page and fill in the form"
- "Scrape the pricing table from this web page"
