//! Provider factory — builds the configured AI provider from config.

use omega_core::{config, traits::Provider};
use omega_providers::{
    anthropic::AnthropicProvider, claude_code::ClaudeCodeProvider, gemini::GeminiProvider,
    ollama::OllamaProvider, openai::OpenAiProvider, openrouter::OpenRouterProvider,
};

/// Build the configured provider, returning `(provider, model_fast, model_complex)`.
///
/// For Claude Code, `model_fast` and `model_complex` come from its config.
/// For all other providers, both are set to the provider's single `model` field.
pub fn build_provider(
    cfg: &config::Config,
    workspace_path: &std::path::Path,
) -> anyhow::Result<(Box<dyn Provider>, String, String)> {
    let ws = Some(workspace_path.to_path_buf());

    match cfg.provider.default.as_str() {
        "claude-code" => {
            let cc = cfg
                .provider
                .claude_code
                .as_ref()
                .cloned()
                .unwrap_or_default();
            let model_fast = cc.model.clone();
            let model_complex = cc.model_complex.clone();
            Ok((
                Box::new(ClaudeCodeProvider::from_config(
                    cc.max_turns,
                    cc.allowed_tools,
                    cc.timeout_secs,
                    ws,
                    cc.max_resume_attempts,
                    cc.model,
                )),
                model_fast,
                model_complex,
            ))
        }
        "ollama" => {
            let oc = cfg
                .provider
                .ollama
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("provider.ollama section missing in config"))?;
            let m = oc.model.clone();
            Ok((
                Box::new(OllamaProvider::from_config(
                    oc.base_url.clone(),
                    oc.model.clone(),
                    ws,
                )?),
                m.clone(),
                m,
            ))
        }
        "openai" => {
            let oc = cfg
                .provider
                .openai
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("provider.openai section missing in config"))?;
            let m = oc.model.clone();
            Ok((
                Box::new(OpenAiProvider::from_config(
                    oc.base_url.clone(),
                    oc.api_key.clone(),
                    oc.model.clone(),
                    ws,
                )?),
                m.clone(),
                m,
            ))
        }
        "anthropic" => {
            let ac =
                cfg.provider.anthropic.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("provider.anthropic section missing in config")
                })?;
            let m = ac.model.clone();
            Ok((
                Box::new(AnthropicProvider::from_config(
                    ac.api_key.clone(),
                    ac.model.clone(),
                    ac.max_tokens,
                    ws,
                )?),
                m.clone(),
                m,
            ))
        }
        "openrouter" => {
            let oc =
                cfg.provider.openrouter.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("provider.openrouter section missing in config")
                })?;
            let m = oc.model.clone();
            Ok((
                Box::new(OpenRouterProvider::from_config(
                    oc.api_key.clone(),
                    oc.model.clone(),
                    ws,
                )?),
                m.clone(),
                m,
            ))
        }
        "gemini" => {
            let gc = cfg
                .provider
                .gemini
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("provider.gemini section missing in config"))?;
            let m = gc.model.clone();
            Ok((
                Box::new(GeminiProvider::from_config(
                    gc.api_key.clone(),
                    gc.model.clone(),
                    ws,
                )?),
                m.clone(),
                m,
            ))
        }
        other => anyhow::bail!("unsupported provider: {other}"),
    }
}
