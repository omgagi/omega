mod gateway;

use clap::{Parser, Subcommand};
use omega_channels::telegram::TelegramChannel;
use omega_core::{config, context::Context, traits::Provider};
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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
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

            // Build provider.
            let provider = build_provider(&cfg)?;

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

            if channels.is_empty() {
                anyhow::bail!("No channels enabled. Enable at least one channel in config.toml.");
            }

            // Build memory.
            let memory = Store::new(&cfg.memory).await?;

            // Build and run gateway.
            println!("Ω Omega — Starting agent...");
            let gw = gateway::Gateway::new(
                provider,
                channels,
                memory,
                cfg.auth.clone(),
                cfg.channel.clone(),
            );
            gw.run().await?;
        }
        Commands::Status => {
            let cfg = config::load(&cli.config)?;
            println!("Ω Omega — Status Check\n");
            println!("Config: {}", cli.config);
            println!("Default provider: {}", cfg.provider.default);
            println!();

            let available = ClaudeCodeProvider::check_cli().await;
            println!(
                "  claude-code: {}",
                if available { "available" } else { "not found" }
            );
            println!();

            // Check channels.
            if let Some(ref tg) = cfg.channel.telegram {
                println!(
                    "  telegram: {}",
                    if tg.enabled && !tg.bot_token.is_empty() {
                        "configured"
                    } else if tg.enabled {
                        "enabled but missing bot_token"
                    } else {
                        "disabled"
                    }
                );
            } else {
                println!("  telegram: not configured");
            }
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
            )))
        }
        other => anyhow::bail!("unsupported provider: {other}"),
    }
}
