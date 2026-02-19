mod commands;
mod gateway;
mod init;
mod selfcheck;
mod service;

use clap::{Parser, Subcommand};
use omega_channels::telegram::TelegramChannel;
use omega_channels::whatsapp::WhatsAppChannel;
use omega_core::{
    config::{self, shellexpand, Prompts},
    context::Context,
    traits::Provider,
};
use omega_memory::Store;
use omega_providers::{
    anthropic::AnthropicProvider, claude_code::ClaudeCodeProvider, gemini::GeminiProvider,
    ollama::OllamaProvider, openai::OpenAiProvider, openrouter::OpenRouterProvider,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Parser)]
#[command(
    name = "omega",
    version,
    about = "OMEGA Ω — Personal AI Agent Infrastructure"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to config file.
    #[arg(short, long, default_value = "config.toml")]
    config: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the OMEGA Ω agent.
    Start,
    /// Check system health and provider availability.
    Status,
    /// Send a one-shot message to the agent.
    Ask {
        /// The message to send.
        #[arg(trailing_var_arg = true)]
        message: Vec<String>,
    },
    /// Interactive setup wizard.
    Init,
    /// Manage the system service (install, uninstall, status).
    Service {
        #[command(subcommand)]
        action: ServiceAction,
    },
}

#[derive(Subcommand)]
enum ServiceAction {
    /// Install Omega as a system service.
    Install,
    /// Remove the Omega system service.
    Uninstall,
    /// Check service installation and running status.
    Status,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Refuse to run as root — claude CLI rejects root for security.
    if unsafe { libc::geteuid() } == 0 {
        anyhow::bail!(
            "Omega must not run as root. Use a LaunchAgent (~/Library/LaunchAgents/) \
             instead of a LaunchDaemon (/Library/LaunchDaemons/)."
        );
    }

    match cli.command {
        Commands::Start => {
            let mut cfg = config::load(&cli.config)?;

            // Set up logging: stdout + file appender to {data_dir}/omega.log.
            let data_dir = shellexpand(&cfg.omega.data_dir);
            let file_appender = tracing_appender::rolling::never(&data_dir, "omega.log");
            let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

            let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

            tracing_subscriber::registry()
                .with(env_filter)
                .with(tracing_subscriber::fmt::layer()) // stdout
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_writer(non_blocking)
                        .with_ansi(false), // no color codes in file
                )
                .init();

            // Env var override: OPENAI_API_KEY → whisper_api_key (if not set in config).
            if let Some(ref mut tg) = cfg.channel.telegram {
                let has_key = tg.whisper_api_key.as_ref().is_some_and(|k| !k.is_empty());
                if !has_key {
                    if let Ok(key) = std::env::var("OPENAI_API_KEY") {
                        if !key.is_empty() {
                            tg.whisper_api_key = Some(key);
                        }
                    }
                }
            }

            // Deploy bundled prompts (SYSTEM_PROMPT.md, WELCOME.toml) on first run.
            config::install_bundled_prompts(&cfg.omega.data_dir);
            let mut prompts = Prompts::load(&cfg.omega.data_dir);

            // Deploy bundled skills, migrate legacy flat files, then load from ~/.omega/skills/*/SKILL.md.
            omega_skills::install_bundled_skills(&cfg.omega.data_dir);
            omega_skills::migrate_flat_skills(&cfg.omega.data_dir);
            let skills = omega_skills::load_skills(&cfg.omega.data_dir);
            let skill_block = omega_skills::build_skill_prompt(&skills);
            if !skill_block.is_empty() {
                prompts.system.push_str(&skill_block);
            }

            // Ensure projects dir exists (projects are hot-reloaded per message).
            omega_skills::ensure_projects_dir(&cfg.omega.data_dir);

            // Create workspace directory for sandbox isolation.
            let workspace_path = {
                let expanded = shellexpand(&cfg.omega.data_dir);
                let ws = PathBuf::from(&expanded).join("workspace");
                if let Err(e) = std::fs::create_dir_all(&ws) {
                    anyhow::bail!("failed to create workspace {}: {e}", ws.display());
                }
                ws
            };

            // Extract model config before building the provider (which consumes cc).
            let cc = cfg
                .provider
                .claude_code
                .as_ref()
                .cloned()
                .unwrap_or_default();
            let model_fast = cc.model.clone();
            let model_complex = cc.model_complex.clone();

            // Build provider with workspace as working directory.
            let provider: Arc<dyn omega_core::traits::Provider> =
                Arc::from(build_provider(&cfg, &workspace_path)?);

            tracing::info!(
                "sandbox mode: {} | workspace: {}",
                cfg.sandbox.mode.display_name(),
                workspace_path.display()
            );

            if !provider.is_available().await {
                anyhow::bail!("provider '{}' is not available", provider.name());
            }

            // Build channels.
            let mut channels: HashMap<String, Arc<dyn omega_core::traits::Channel>> =
                HashMap::new();

            if let Some(ref tg) = cfg.channel.telegram {
                if tg.enabled {
                    if tg.bot_token.is_empty() {
                        anyhow::bail!(
                            "Telegram is enabled but bot_token is empty. \
                             Set it in config.toml or TELEGRAM_BOT_TOKEN env var."
                        );
                    }
                    let channel = TelegramChannel::new(tg.clone());
                    channels.insert("telegram".to_string(), Arc::new(channel));
                }
            }

            if let Some(ref wa) = cfg.channel.whatsapp {
                if wa.enabled {
                    let channel = WhatsAppChannel::new(wa.clone(), &cfg.omega.data_dir);
                    channels.insert("whatsapp".to_string(), Arc::new(channel));
                }
            }

            if channels.is_empty() {
                anyhow::bail!("No channels enabled. Enable at least one channel in config.toml.");
            }

            // Build memory.
            let memory = Store::new(&cfg.memory).await?;

            // Self-check before starting.
            if !selfcheck::run(&cfg, &memory).await {
                anyhow::bail!("Self-check failed. Fix the issues above before starting.");
            }

            // Compute sandbox prompt constraint.
            let sandbox_mode = cfg.sandbox.mode;
            let sandbox_prompt = sandbox_mode.prompt_constraint(&workspace_path.to_string_lossy());

            // Build and run gateway.
            println!("OMEGA Ω — Starting agent...");
            let gw = Arc::new(gateway::Gateway::new(
                provider,
                channels,
                memory,
                cfg.auth.clone(),
                cfg.channel.clone(),
                cfg.heartbeat.clone(),
                cfg.scheduler.clone(),
                prompts,
                cfg.omega.data_dir.clone(),
                skills,
                sandbox_mode.display_name().to_string(),
                sandbox_prompt,
                model_fast,
                model_complex,
            ));
            gw.run().await?;
        }
        Commands::Status => {
            init_stdout_tracing("error");
            let cfg = config::load(&cli.config)?;
            cliclack::intro(console::style("omega status").bold().to_string())?;

            cliclack::log::info(format!(
                "Config: {}\nProvider: {}",
                cli.config, cfg.provider.default
            ))?;

            let available = ClaudeCodeProvider::check_cli().await;
            if available {
                cliclack::log::success("claude-code — available")?;
            } else {
                cliclack::log::error("claude-code — not found")?;
            }

            // Check channels.
            if let Some(ref tg) = cfg.channel.telegram {
                let status = if tg.enabled && !tg.bot_token.is_empty() {
                    "configured"
                } else if tg.enabled {
                    "enabled but missing bot_token"
                } else {
                    "disabled"
                };
                cliclack::log::info(format!("telegram — {status}"))?;
            } else {
                cliclack::log::info("telegram — not configured")?;
            }

            if let Some(ref wa) = cfg.channel.whatsapp {
                let status = if wa.enabled { "enabled" } else { "disabled" };
                cliclack::log::info(format!("whatsapp — {status}"))?;
            } else {
                cliclack::log::info("whatsapp — not configured")?;
            }

            cliclack::outro("Status check complete")?;
        }
        Commands::Ask { message } => {
            init_stdout_tracing("info");
            if message.is_empty() {
                anyhow::bail!("no message provided. Usage: omega ask <message>");
            }

            let prompt = message.join(" ");
            let cfg = config::load(&cli.config)?;

            // Ensure workspace exists for ask command too.
            let workspace_path = {
                let expanded = shellexpand(&cfg.omega.data_dir);
                let ws = PathBuf::from(&expanded).join("workspace");
                let _ = std::fs::create_dir_all(&ws);
                ws
            };

            let provider = build_provider(&cfg, &workspace_path)?;

            if !provider.is_available().await {
                anyhow::bail!(
                    "provider '{}' is not available. Is the claude CLI installed and authenticated?",
                    provider.name()
                );
            }

            let context = Context::new(&prompt);
            let response = provider.complete(&context).await?;
            println!("{}", response.text);
        }
        Commands::Init => {
            init_stdout_tracing("error");
            init::run().await?;
        }
        Commands::Service { action } => {
            init_stdout_tracing("error");
            match action {
                ServiceAction::Install => service::install(&cli.config)?,
                ServiceAction::Uninstall => service::uninstall()?,
                ServiceAction::Status => service::status()?,
            }
        }
    }

    Ok(())
}

/// Set up stdout-only tracing for non-daemon commands.
fn init_stdout_tracing(level: &str) {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level)),
        )
        .init();
}

/// Build the configured provider.
fn build_provider(
    cfg: &config::Config,
    workspace_path: &std::path::Path,
) -> anyhow::Result<Box<dyn Provider>> {
    match cfg.provider.default.as_str() {
        "claude-code" => {
            let cc = cfg
                .provider
                .claude_code
                .as_ref()
                .cloned()
                .unwrap_or_default();
            Ok(Box::new(ClaudeCodeProvider::from_config(
                cc.max_turns,
                cc.allowed_tools,
                cc.timeout_secs,
                Some(workspace_path.to_path_buf()),
                cfg.sandbox.mode,
                cc.max_resume_attempts,
                cc.model,
            )))
        }
        "ollama" => {
            let oc = cfg
                .provider
                .ollama
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("provider.ollama section missing in config"))?;
            Ok(Box::new(OllamaProvider::from_config(
                oc.base_url.clone(),
                oc.model.clone(),
            )))
        }
        "openai" => {
            let oc = cfg
                .provider
                .openai
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("provider.openai section missing in config"))?;
            Ok(Box::new(OpenAiProvider::from_config(
                oc.base_url.clone(),
                oc.api_key.clone(),
                oc.model.clone(),
            )))
        }
        "anthropic" => {
            let ac =
                cfg.provider.anthropic.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("provider.anthropic section missing in config")
                })?;
            Ok(Box::new(AnthropicProvider::from_config(
                ac.api_key.clone(),
                ac.model.clone(),
            )))
        }
        "openrouter" => {
            let oc =
                cfg.provider.openrouter.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("provider.openrouter section missing in config")
                })?;
            Ok(Box::new(OpenRouterProvider::from_config(
                oc.api_key.clone(),
                oc.model.clone(),
            )))
        }
        "gemini" => {
            let gc = cfg
                .provider
                .gemini
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("provider.gemini section missing in config"))?;
            Ok(Box::new(GeminiProvider::from_config(
                gc.api_key.clone(),
                gc.model.clone(),
            )))
        }
        other => anyhow::bail!("unsupported provider: {other}"),
    }
}
