use clap::{Parser, Subcommand};
use omega_core::{config, context::Context, traits::Provider};
use omega_providers::claude_code::ClaudeCodeProvider;

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

    match cli.command {
        Commands::Start => {
            println!("Ω Omega — Starting agent...");
            println!("(Not yet implemented. Use `omega ask` for now.)");
        }
        Commands::Status => {
            let cfg = config::load(&cli.config)?;
            println!("Ω Omega — Status Check\n");
            println!("Config: {}", cli.config);
            println!("Default provider: {}", cfg.provider.default);
            println!();

            // Check Claude Code CLI availability.
            let available = ClaudeCodeProvider::check_cli().await;
            println!(
                "  claude-code: {}",
                if available { "available" } else { "not found" }
            );
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
