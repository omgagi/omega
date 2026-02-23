mod api;
mod claudemd;
mod commands;
mod gateway;
mod i18n;
mod init;
mod init_wizard;
mod markers;
mod pair;
mod provider_builder;
mod selfcheck;
mod service;
mod task_confirmation;

use clap::{Parser, Subcommand};
use omega_channels::telegram::TelegramChannel;
use omega_channels::whatsapp::WhatsAppChannel;
use omega_core::config::{self, shellexpand, Prompts};
use omega_core::context::Context;
use omega_memory::Store;
use omega_providers::claude_code::ClaudeCodeProvider;
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
    /// Interactive setup wizard (or non-interactive with --telegram-token).
    Init {
        /// Telegram bot token (from @BotFather). Enables non-interactive mode.
        #[arg(long, env = "OMEGA_TELEGRAM_TOKEN")]
        telegram_token: Option<String>,

        /// Comma-separated allowed Telegram user IDs.
        #[arg(long, env = "OMEGA_ALLOWED_USERS")]
        allowed_users: Option<String>,

        /// Claude CLI setup-token for fresh machines.
        #[arg(long, env = "OMEGA_CLAUDE_SETUP_TOKEN")]
        claude_setup_token: Option<String>,

        /// OpenAI API key for Whisper voice transcription.
        #[arg(long, env = "OMEGA_WHISPER_KEY")]
        whisper_key: Option<String>,

        /// Path to Google OAuth client_secret.json file.
        #[arg(long, env = "OMEGA_GOOGLE_CREDENTIALS")]
        google_credentials: Option<String>,

        /// Gmail address for Google Workspace integration.
        #[arg(long, env = "OMEGA_GOOGLE_EMAIL")]
        google_email: Option<String>,
    },
    /// Pair WhatsApp by scanning a QR code.
    Pair,
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
        Commands::Start => cmd_start(&cli.config).await?,
        Commands::Status => cmd_status(&cli.config).await?,
        Commands::Ask { message } => cmd_ask(&cli.config, message).await?,
        Commands::Init {
            telegram_token,
            allowed_users,
            claude_setup_token,
            whisper_key,
            google_credentials,
            google_email,
        } => {
            init_stdout_tracing("error");
            if telegram_token.is_some() || allowed_users.is_some() {
                init::run_noninteractive(
                    telegram_token.as_deref().unwrap_or(""),
                    allowed_users.as_deref().unwrap_or(""),
                    claude_setup_token.as_deref(),
                    whisper_key.as_deref(),
                    google_credentials.as_deref(),
                    google_email.as_deref(),
                )?;
            } else {
                init::run().await?;
            }
        }
        Commands::Pair => {
            init_stdout_tracing("error");
            pair::pair_whatsapp().await?;
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

/// Start the OMEGA agent.
async fn cmd_start(config_path: &str) -> anyhow::Result<()> {
    let mut cfg = config::load(config_path)?;

    // Migrate flat ~/.omega/ layout to structured subdirectories.
    config::migrate_layout(&cfg.omega.data_dir, config_path);

    // Set up logging: stdout + file appender to {data_dir}/logs/omega.log.
    let data_dir = shellexpand(&cfg.omega.data_dir);
    let log_dir = PathBuf::from(&data_dir).join("logs");
    let _ = std::fs::create_dir_all(&log_dir);
    let file_appender = tracing_appender::rolling::never(&log_dir, "omega.log");
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

    // Deploy bundled skills, migrate legacy flat files, then load.
    omega_skills::install_bundled_skills(&cfg.omega.data_dir);
    omega_skills::migrate_flat_skills(&cfg.omega.data_dir);
    let skills = omega_skills::load_skills(&cfg.omega.data_dir);
    let skill_block = omega_skills::build_skill_prompt(&skills);
    if !skill_block.is_empty() {
        prompts.system.push_str(&skill_block);
    }

    // Ensure projects dir exists (projects are hot-reloaded per message).
    omega_skills::ensure_projects_dir(&cfg.omega.data_dir);

    // Create workspace directory.
    let workspace_path = {
        let expanded = shellexpand(&cfg.omega.data_dir);
        let ws = PathBuf::from(&expanded).join("workspace");
        if let Err(e) = std::fs::create_dir_all(&ws) {
            anyhow::bail!("failed to create workspace {}: {e}", ws.display());
        }
        ws
    };

    // Build provider with workspace as working directory.
    let (provider_box, model_fast, model_complex) =
        provider_builder::build_provider(&cfg, &workspace_path)?;
    let provider: Arc<dyn omega_core::traits::Provider> = Arc::from(provider_box);

    tracing::info!("workspace: {}", workspace_path.display());

    if !provider.is_available().await {
        anyhow::bail!("provider '{}' is not available", provider.name());
    }

    // Build channels.
    let mut channels: HashMap<String, Arc<dyn omega_core::traits::Channel>> = HashMap::new();

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

    // Create stores directory for domain-specific databases.
    {
        let stores_dir = PathBuf::from(&shellexpand(&cfg.omega.data_dir)).join("stores");
        let _ = std::fs::create_dir_all(&stores_dir);
    }

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
        cfg.api.clone(),
        prompts,
        cfg.omega.data_dir.clone(),
        skills,
        model_fast,
        model_complex,
        config_path.to_string(),
    ));
    gw.run().await
}

/// Check system health and provider availability.
async fn cmd_status(config_path: &str) -> anyhow::Result<()> {
    init_stdout_tracing("error");
    let cfg = config::load(config_path)?;
    cliclack::intro(console::style("omega status").bold().to_string())?;

    cliclack::log::info(format!(
        "Config: {config_path}\nProvider: {}",
        cfg.provider.default
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
    Ok(())
}

/// Send a one-shot message to the agent.
async fn cmd_ask(config_path: &str, message: Vec<String>) -> anyhow::Result<()> {
    init_stdout_tracing("info");
    if message.is_empty() {
        anyhow::bail!("no message provided. Usage: omega ask <message>");
    }

    let prompt = message.join(" ");
    let cfg = config::load(config_path)?;

    // Ensure workspace exists for ask command too.
    let workspace_path = {
        let expanded = shellexpand(&cfg.omega.data_dir);
        let ws = PathBuf::from(&expanded).join("workspace");
        let _ = std::fs::create_dir_all(&ws);
        ws
    };

    let (provider, _model_fast, _model_complex) =
        provider_builder::build_provider(&cfg, &workspace_path)?;

    if !provider.is_available().await {
        anyhow::bail!(
            "provider '{}' is not available. Is the claude CLI installed and authenticated?",
            provider.name()
        );
    }

    let context = Context::new(&prompt);
    let response = provider.complete(&context).await?;
    println!("{}", response.text);
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
