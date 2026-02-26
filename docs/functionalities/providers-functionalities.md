# Functionalities: Providers

## Overview
Six AI provider implementations behind a common `Provider` trait. Claude Code CLI is the default (subprocess-based). The other five are HTTP-based with a shared agentic tool loop. All HTTP providers support 4 built-in tools (Bash, Read, Write, Edit) plus MCP server routing.

## Functionalities

| # | Name | Type | Location | Description | Dependencies |
|---|------|------|----------|-------------|--------------|
| 1 | Provider trait | Trait | backend/crates/omega-core/src/traits.rs:~5 | name(), requires_api_key(), complete(Context) -> OutgoingMessage, is_available() | -- |
| 2 | build_provider() | Function | backend/src/provider_builder.rs:~10 | Factory for 6 providers; returns (Box<dyn Provider>, model_fast, model_complex); Claude Code: fast=Sonnet, complex=Opus | Config |
| 3 | ClaudeCodeProvider | Struct | backend/crates/omega-providers/src/claude_code/provider.rs:~10 | CLI subprocess provider with auto-resume, MCP settings lifecycle, configurable timeout | -- |
| 4 | ClaudeCodeProvider::complete() | Method | backend/crates/omega-providers/src/claude_code/provider.rs:~30 | Writes MCP settings, runs CLI, parses JSON response, handles auto-resume (exponential backoff: 2s, 4s, 8s), returns session_id | Command, MCP |
| 5 | build_run_cli_args() | Method | backend/crates/omega-providers/src/claude_code/command.rs:15 | Builds CLI args: agent mode (--agent), session (--resume), tool permissions, model override, max_turns | -- |
| 6 | run_cli() | Method | backend/crates/omega-providers/src/claude_code/command.rs:96 | Runs claude CLI subprocess with timeout | Command |
| 7 | run_cli_with_session() | Method | backend/crates/omega-providers/src/claude_code/command.rs:126 | Runs claude CLI with specific session ID (for auto-resume) | Command |
| 8 | base_command() | Method | backend/crates/omega-providers/src/claude_code/command.rs:172 | Builds base Command with working directory and sandbox protection via omega_sandbox::protected_command | Sandbox |
| 9 | execute_with_timeout() | Method | backend/crates/omega-providers/src/claude_code/command.rs:190 | Executes command with configurable timeout and standard error handling | -- |
| 10 | OpenAiProvider | Struct | backend/crates/omega-providers/src/openai.rs:24 | OpenAI-compatible API with agentic tool loop | Tools |
| 11 | openai_agentic_complete() | Function | backend/crates/omega-providers/src/openai.rs:165 | Shared agentic loop: infer -> tool calls -> execute -> feed back; used by OpenAI + OpenRouter | Tools |
| 12 | AnthropicProvider | Struct | backend/crates/omega-providers/src/anthropic.rs:22 | Anthropic Messages API with content blocks (text/tool_use/tool_result) | Tools |
| 13 | OllamaProvider | Struct | backend/crates/omega-providers/src/ollama.rs:19 | Local Ollama server; no API key; tool calling without tool_call_id | Tools |
| 14 | OpenRouterProvider | Struct | backend/crates/omega-providers/src/openrouter.rs:23 | OpenRouter proxy; reuses OpenAI types and agentic loop | OpenAI, tools |
| 15 | GeminiProvider | Struct | backend/crates/omega-providers/src/gemini.rs:21 | Google Gemini API; functionCall/functionResponse parts; role mapping (assistant->model) | Tools |
| 16 | ToolExecutor | Struct | backend/crates/omega-providers/src/tools.rs:43 | 4 built-in tools (Bash, Read, Write, Edit) + MCP server routing; sandbox enforcement | Sandbox, MCP |
| 17 | ToolExecutor::execute() | Method | backend/crates/omega-providers/src/tools.rs:107 | Routes tool calls to built-in or MCP; sandbox checks on read/write paths | Sandbox |
| 18 | exec_bash() | Method | backend/crates/omega-providers/src/tools.rs:154 | Bash tool: sandboxed command, 120s timeout, 30KB output limit | Sandbox |
| 19 | exec_read() | Method | backend/crates/omega-providers/src/tools.rs:209 | Read tool: sandbox-checked, 50KB output limit | Sandbox |
| 20 | exec_write() | Method | backend/crates/omega-providers/src/tools.rs:240 | Write tool: sandbox-checked, creates parent dirs | Sandbox |
| 21 | exec_edit() | Method | backend/crates/omega-providers/src/tools.rs:282 | Edit tool: sandbox-checked, find-and-replace first occurrence | Sandbox |
| 22 | McpClient | Struct | backend/crates/omega-providers/src/mcp_client.rs:36 | Minimal MCP client over stdio; JSON-RPC 2.0; initialize handshake + tools/list discovery | -- |
| 23 | McpClient::call_tool() | Method | backend/crates/omega-providers/src/mcp_client.rs:148 | Calls a tool on the MCP server via JSON-RPC | -- |
| 24 | connect_mcp_servers() | Method | backend/crates/omega-providers/src/tools.rs:70 | Connects to MCP servers, discovers tools, builds routing map | MCP |

## Internal Dependencies
- build_provider() -> each provider's from_config()
- ClaudeCodeProvider::complete() -> build_run_cli_args() -> run_cli() -> execute_with_timeout()
- All HTTP providers -> ToolExecutor -> exec_bash/read/write/edit + MCP routing
- openai_agentic_complete() reused by OpenRouterProvider
- ToolExecutor -> omega_sandbox::is_write_blocked/is_read_blocked
- ToolExecutor -> McpClient for external tool servers

## Dead Code / Unused
None detected.
