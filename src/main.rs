mod commands;
mod gateway;
mod init;
mod selfcheck;
mod service;

use clap::{Parser, Subcommand};
use omega_channels::telegram::TelegramChannel;
use omega_channels::whatsapp::WhatsAppChannel;
use omega_core::{
    config::{self, Prompts},
    context::Context,
    traits::Provider,
};
use omega_memory::Store;
use omega_providers::claude_code::ClaudeCodeProvider;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Parser)]
#[command(
    name = "omega",
    version,
    about = "Ω Omega — Personal AI Agent Infrastructure"
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
    /// Start the Omega agent.
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

    // Only enable verbose logging for commands that run the agent.
    // Init and Service are interactive CLI flows — no backend noise.
    let log_level = match cli.command {
        Commands::Init | Commands::Service { .. } => "error",
        _ => "info",
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level)),
        )
        .init();

    // Refuse to run as root — claude CLI rejects root for security.
    if unsafe { libc::geteuid() } == 0 {
        anyhow::bail!(
            "Omega must not run as root. Use a LaunchAgent (~/Library/LaunchAgents/) \
             instead of a LaunchDaemon (/Library/LaunchDaemons/)."
        );
    }

    match cli.command {
        Commands::Start => {
            let cfg = config::load(&cli.config)?;
            let mut prompts = Prompts::load(&cfg.omega.data_dir);

            // Deploy bundled skills, then load all from ~/.omega/skills/*.md.
            omega_skills::install_bundled_skills(&cfg.omega.data_dir);
            let skills = omega_skills::load_skills(&cfg.omega.data_dir);
            let skill_block = omega_skills::build_skill_prompt(&skills);
            if !skill_block.is_empty() {
                prompts.system.push_str(&skill_block);
            }

            // Build provider.
            let provider: Arc<dyn omega_core::traits::Provider> = Arc::from(build_provider(&cfg)?);

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

            // Build and run gateway.
            println!("Ω Omega — Starting agent...");
            let mut gw = gateway::Gateway::new(
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
            );
            gw.run().await?;
        }
        Commands::Status => {
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
            if message.is_empty() {
                anyhow::bail!("no message provided. Usage: omega ask <message>");
            }

            let prompt = message.join(" ");
            let cfg = config::load(&cli.config)?;
            let provider = build_provider(&cfg)?;

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
            init::run().await?;
        }
        Commands::Service { action } => match action {
            ServiceAction::Install => service::install(&cli.config)?,
            ServiceAction::Uninstall => service::uninstall()?,
            ServiceAction::Status => service::status()?,
        },
    }

    Ok(())
}

/// Build the configured provider.
fn build_provider(cfg: &config::Config) -> anyhow::Result<Box<dyn Provider>> {
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
            )))
        }
        other => anyhow::bail!("unsupported provider: {other}"),
    }
}
