//! Init wizard — interactive setup for new users with cliclack styled prompts,
//! plus non-interactive mode for programmatic deployment.

use crate::init_wizard;
use crate::service;
use omega_core::shellexpand;
use std::path::Path;

const LOGO: &str = r#"
              ██████╗ ███╗   ███╗███████╗ ██████╗  █████╗        █████╗
             ██╔═══██╗████╗ ████║██╔════╝██╔════╝ ██╔══██╗      ██╔══██╗
             ██║   ██║██╔████╔██║█████╗  ██║  ███╗███████║      ██║  ██║
             ██║   ██║██║╚██╔╝██║██╔══╝  ██║   ██║██╔══██║      ╚██╗██╔╝
             ╚██████╔╝██║ ╚═╝ ██║███████╗╚██████╔╝██║  ██║    ████╔╝╚████╗
              ╚═════╝ ╚═╝     ╚═╝╚══════╝ ╚═════╝ ╚═╝  ╚═╝    ╚═══╝  ╚═══╝
"#;

/// Run the interactive init wizard.
pub async fn run() -> anyhow::Result<()> {
    println!("{LOGO}");
    cliclack::intro("omega init")?;

    // 1. Create data directory.
    let data_dir = shellexpand("~/.omega");
    if !Path::new(&data_dir).exists() {
        std::fs::create_dir_all(&data_dir)?;
        cliclack::log::success(format!("{data_dir} — created"))?;
    } else {
        cliclack::log::success(format!("{data_dir} — exists"))?;
    }

    // 2. Check claude CLI.
    let spinner = cliclack::spinner();
    spinner.start("Checking claude CLI...");
    let claude_ok = std::process::Command::new("claude")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if claude_ok {
        spinner.stop("claude CLI — found");
    } else {
        spinner.error("claude CLI — NOT FOUND");
        cliclack::note(
            "Install claude CLI",
            "npm install -g @anthropic-ai/claude-code\n\nThen run 'omega init' again.",
        )?;
        cliclack::outro_cancel("Setup aborted")?;
        return Ok(());
    }

    // 3. Anthropic authentication.
    init_wizard::run_anthropic_auth()?;

    // 4. Telegram bot token.
    let bot_token: String = cliclack::input("Telegram bot token")
        .placeholder("Paste token from @BotFather (or Enter to skip)")
        .required(false)
        .default_input("")
        .interact()?;

    if bot_token.is_empty() {
        cliclack::log::info("Skipping Telegram — you can add it later in config.toml")?;
    }

    // 5. User ID (optional).
    let user_id: Option<i64> = if !bot_token.is_empty() {
        let id_str: String = cliclack::input("Your Telegram user ID")
            .placeholder("Send /start to @userinfobot (blank = allow all)")
            .required(false)
            .default_input("")
            .interact()?;
        id_str.parse::<i64>().ok()
    } else {
        None
    };

    // 6. Voice transcription (Whisper).
    let whisper_api_key: Option<String> = if !bot_token.is_empty() {
        let setup: bool = cliclack::confirm("Enable voice message transcription?")
            .initial_value(false)
            .interact()?;
        if setup {
            cliclack::note(
                "Voice Transcription",
                "Voice messages will be transcribed using OpenAI Whisper.\n\
                 Get a key: https://platform.openai.com/api-keys",
            )?;
            let key: String = cliclack::input("OpenAI API key (for Whisper)")
                .placeholder("sk-... (Enter to skip, or set OPENAI_API_KEY later)")
                .required(false)
                .default_input("")
                .interact()?;
            if key.is_empty() {
                None
            } else {
                Some(key)
            }
        } else {
            None
        }
    } else {
        None
    };

    // 7. WhatsApp setup.
    let whatsapp_enabled = init_wizard::run_whatsapp_setup().await?;

    // 8. Google Workspace setup.
    let google_email = init_wizard::run_google_setup()?;

    // 9. Generate config.toml.
    let config_path = "config.toml";
    if Path::new(config_path).exists() {
        cliclack::log::warning(
            "config.toml already exists — skipping.\nDelete it and run 'omega init' again to regenerate.",
        )?;
    } else {
        let user_ids: Vec<i64> = user_id.into_iter().collect();
        let config = generate_config(
            &bot_token,
            &user_ids,
            whisper_api_key.as_deref(),
            whatsapp_enabled,
            google_email.as_deref(),
        );
        std::fs::write(config_path, config)?;
        cliclack::log::success("Generated config.toml")?;
    }

    // 11. Offer service installation.
    let install_service: bool = cliclack::confirm("Install Omega as a system service?")
        .initial_value(true)
        .interact()?;

    let service_installed = if install_service {
        match service::install(config_path) {
            Ok(()) => true,
            Err(e) => {
                cliclack::log::warning(format!("Service install failed: {e}"))?;
                cliclack::log::info("You can install later with: omega service install")?;
                false
            }
        }
    } else {
        false
    };

    // 12. Next steps.
    let mut steps =
        String::from("1. Review config.toml\n2. Run: omega start\n3. Send a message to your bot");
    if whatsapp_enabled {
        steps.push_str("\n4. WhatsApp is linked and ready!");
    }
    if google_email.is_some() {
        steps.push_str("\n★ Google Workspace is connected!");
    }
    if service_installed {
        steps.push_str("\n★ System service installed — OMEGA Ω starts on login!");
    } else if !install_service {
        steps.push_str("\nTip: Run `omega service install` to auto-start on login");
    }
    cliclack::note("Next steps", &steps)?;

    cliclack::outro("Setup complete — enjoy OMEGA Ω!")?;
    Ok(())
}

/// Parse a comma-separated string of user IDs into a Vec.
pub fn parse_allowed_users(csv: &str) -> anyhow::Result<Vec<i64>> {
    if csv.trim().is_empty() {
        return Ok(Vec::new());
    }
    csv.split(',')
        .map(|s| {
            s.trim()
                .parse::<i64>()
                .map_err(|_| anyhow::anyhow!("invalid user ID: '{}'", s.trim()))
        })
        .collect()
}

/// Run non-interactive init for programmatic deployment.
///
/// Generates config, deploys bundled prompts/skills, creates workspace,
/// installs system service — all without interactive prompts.
pub fn run_noninteractive(
    telegram_token: &str,
    allowed_users_csv: &str,
    claude_setup_token: Option<&str>,
    whisper_key: Option<&str>,
    google_credentials: Option<&str>,
    google_email: Option<&str>,
) -> anyhow::Result<()> {
    println!("OMEGA Ω — non-interactive init");

    // 1. Validate inputs.
    let user_ids = parse_allowed_users(allowed_users_csv)?;

    // 2. Create data directory.
    let data_dir = shellexpand("~/.omega");
    std::fs::create_dir_all(&data_dir)?;
    println!("  Data directory: {data_dir}");

    // 3. Check claude CLI (warn if missing, non-fatal).
    let claude_ok = std::process::Command::new("claude")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if claude_ok {
        println!("  claude CLI: found");
    } else {
        println!("  WARNING: claude CLI not found — install with: npm install -g @anthropic-ai/claude-code");
    }

    // 4. Apply Claude setup-token if provided.
    if let Some(token) = claude_setup_token {
        let result = std::process::Command::new("claude")
            .args(["setup-token", token.trim()])
            .output();
        match result {
            Ok(output) if output.status.success() => {
                println!("  Claude setup-token: applied");
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                println!("  WARNING: claude setup-token failed: {stderr}");
            }
            Err(e) => {
                println!("  WARNING: could not run claude setup-token: {e}");
            }
        }
    }

    // 5. Google credentials setup (non-interactive part only).
    if let Some(cred_path) = google_credentials {
        let expanded = shellexpand(cred_path);
        if !Path::new(&expanded).exists() {
            println!("  WARNING: Google credentials file not found: {expanded}");
        } else {
            let result = std::process::Command::new("gog")
                .args(["auth", "credentials", &expanded])
                .output();
            match result {
                Ok(output) if output.status.success() => {
                    println!("  Google credentials: registered");
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    println!("  WARNING: gog auth credentials failed: {stderr}");
                }
                Err(e) => {
                    println!("  WARNING: could not run gog: {e}");
                }
            }
        }
        if let Some(email) = google_email {
            println!(
                "  NOTE: Complete Google OAuth post-deployment:\n\
                 \x20   gog auth add {email} --services gmail,calendar,drive,contacts,docs,sheets"
            );
        }
    }

    // 6. Bail if config.toml already exists.
    let config_path = "config.toml";
    if Path::new(config_path).exists() {
        anyhow::bail!("config.toml already exists — delete it first or use a different directory");
    }

    // 7. Generate and write config.toml.
    let config = generate_config(
        telegram_token,
        &user_ids,
        whisper_key,
        false, // WhatsApp handled post-deployment
        google_email,
    );
    std::fs::write(config_path, &config)?;
    println!("  Generated: config.toml");

    // 8. Deploy bundled prompts and skills.
    omega_core::config::install_bundled_prompts("~/.omega");
    println!("  Deployed: bundled prompts");

    omega_skills::install_bundled_skills("~/.omega");
    println!("  Deployed: bundled skills");

    // 9. Create workspace directory.
    let ws = std::path::PathBuf::from(&data_dir).join("workspace");
    std::fs::create_dir_all(&ws)?;
    println!("  Workspace: {}", ws.display());

    // 10. Install system service (non-interactive).
    match service::install_quiet(config_path) {
        Ok(()) => println!("  Service: installed and activated"),
        Err(e) => println!(
            "  WARNING: service install failed: {e}\n  Install later with: omega service install"
        ),
    }

    // 11. Summary.
    println!("\nOMEGA Ω — init complete!");
    println!("  Config: {config_path}");
    println!("  Start: omega start");
    if google_credentials.is_some() && google_email.is_some() {
        println!(
            "  Google OAuth: complete post-deployment with:\n\
             \x20   gog auth add {} --services gmail,calendar,drive,contacts,docs,sheets",
            google_email.unwrap_or("your@email.com")
        );
    }

    Ok(())
}

/// Generate config.toml content from wizard inputs (pure function for testability).
pub fn generate_config(
    bot_token: &str,
    user_ids: &[i64],
    whisper_api_key: Option<&str>,
    whatsapp_enabled: bool,
    google_email: Option<&str>,
) -> String {
    let allowed_users = if user_ids.is_empty() {
        "[]".to_string()
    } else {
        let ids: Vec<String> = user_ids.iter().map(|id| id.to_string()).collect();
        format!("[{}]", ids.join(", "))
    };
    let telegram_enabled = if bot_token.is_empty() {
        "false"
    } else {
        "true"
    };
    let wa_enabled = if whatsapp_enabled { "true" } else { "false" };

    let whisper_line = match whisper_api_key {
        Some(key) if !key.is_empty() => format!("whisper_api_key = \"{key}\""),
        _ if !bot_token.is_empty() => {
            "# whisper_api_key = \"\"  # OpenAI key for voice transcription (or env: OPENAI_API_KEY)"
                .to_string()
        }
        _ => String::new(),
    };

    let mut config = format!(
        r#"[omega]
name = "OMEGA Ω"
data_dir = "~/.omega"
log_level = "info"

[auth]
enabled = true

[provider]
default = "claude-code"

[provider.claude-code]
enabled = true
max_turns = 100
allowed_tools = ["Bash", "Read", "Write", "Edit"]

[channel.telegram]
enabled = {telegram_enabled}
bot_token = "{bot_token}"
allowed_users = {allowed_users}
{whisper_line}

[channel.whatsapp]
enabled = {wa_enabled}
allowed_users = []

[memory]
backend = "sqlite"
db_path = "~/.omega/data/memory.db"
max_context_messages = 50
"#
    );

    if let Some(email) = google_email {
        config.push_str(&format!(
            r#"
[google]
account = "{email}"
"#
        ));
    }

    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_config_full() {
        let config = generate_config(
            "123:ABC",
            &[42],
            Some("sk-key"),
            true,
            Some("me@gmail.com"),
        );
        assert!(config.contains("bot_token = \"123:ABC\""));
        assert!(config.contains("allowed_users = [42]"));
        assert!(
            config.contains("enabled = true"),
            "telegram should be enabled"
        );
        assert!(config.contains("whisper_api_key = \"sk-key\""));
        assert!(config.contains("[channel.whatsapp]\nenabled = true"));
        assert!(config.contains("[google]\naccount = \"me@gmail.com\""));
        assert!(!config.contains("[sandbox]"), "no sandbox section");
    }

    #[test]
    fn test_generate_config_minimal() {
        let config = generate_config("", &[], None, false, None);
        assert!(config.contains("bot_token = \"\""));
        assert!(config.contains("allowed_users = []"));
        assert!(config.contains("[channel.telegram]\nenabled = false"));
        assert!(config.contains("[channel.whatsapp]\nenabled = false"));
        assert!(!config.contains("[google]"));
        assert!(!config.contains("[sandbox]"));
    }

    #[test]
    fn test_generate_config_telegram_only() {
        let config = generate_config("tok:EN", &[999], None, false, None);
        assert!(config.contains("bot_token = \"tok:EN\""));
        assert!(config.contains("allowed_users = [999]"));
        assert!(config.contains("[channel.telegram]\nenabled = true"));
        assert!(config.contains("[channel.whatsapp]\nenabled = false"));
        assert!(!config.contains("[google]"));
    }

    #[test]
    fn test_generate_config_google_only() {
        let config = generate_config("", &[], None, false, Some("test@example.com"));
        assert!(config.contains("[google]\naccount = \"test@example.com\""));
        assert!(config.contains("[channel.telegram]\nenabled = false"));
    }

    #[test]
    fn test_generate_config_whatsapp_only() {
        let config = generate_config("", &[], None, true, None);
        assert!(config.contains("[channel.whatsapp]\nenabled = true"));
        assert!(config.contains("[channel.telegram]\nenabled = false"));
        assert!(!config.contains("[google]"));
    }

    #[test]
    fn test_generate_config_with_whisper() {
        let config = generate_config("tok:EN", &[42], Some("sk-abc"), false, None);
        assert!(config.contains("whisper_api_key = \"sk-abc\""));
    }

    #[test]
    fn test_generate_config_without_whisper() {
        let config = generate_config("tok:EN", &[42], None, false, None);
        assert!(config.contains("# whisper_api_key"));
        assert!(config.contains("OPENAI_API_KEY"));
    }

    #[test]
    fn test_generate_config_multiple_users() {
        let config = generate_config("tok:EN", &[111, 222, 333], None, false, None);
        assert!(config.contains("allowed_users = [111, 222, 333]"));
    }

    #[test]
    fn test_parse_allowed_users_single() {
        let ids = parse_allowed_users("842277204").unwrap();
        assert_eq!(ids, vec![842277204]);
    }

    #[test]
    fn test_parse_allowed_users_multiple() {
        let ids = parse_allowed_users("111, 222, 333").unwrap();
        assert_eq!(ids, vec![111, 222, 333]);
    }

    #[test]
    fn test_parse_allowed_users_empty() {
        let ids = parse_allowed_users("").unwrap();
        assert!(ids.is_empty());
    }

    #[test]
    fn test_parse_allowed_users_whitespace() {
        let ids = parse_allowed_users("  ").unwrap();
        assert!(ids.is_empty());
    }

    #[test]
    fn test_parse_allowed_users_invalid() {
        let result = parse_allowed_users("abc");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid user ID"));
    }

}
