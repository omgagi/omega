//! Init wizard — interactive setup for new users with cliclack styled prompts,
//! plus non-interactive mode for programmatic deployment.

use crate::init_style;
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
///
/// Refuses to run if `~/.omega/config.toml` already exists — use `omega setup`
/// to reconfigure individual components on an existing installation.
pub async fn run() -> anyhow::Result<()> {
    let config_path = shellexpand("~/.omega/config.toml");
    if Path::new(&config_path).exists() {
        init_style::omega_intro(LOGO, "omega init")?;
        init_style::omega_warning("OMEGA is already installed. Use `omega setup` to reconfigure.")?;
        init_style::omega_outro("Nothing changed")?;
        return Ok(());
    }

    init_style::omega_intro(LOGO, "omega init")?;

    // 1. Create data directory.
    let data_dir = shellexpand("~/.omega");
    if !Path::new(&data_dir).exists() {
        std::fs::create_dir_all(&data_dir)?;
        init_style::omega_success(&format!("{data_dir} — created"))?;
    } else {
        init_style::omega_success(&format!("{data_dir} — exists"))?;
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
        init_style::omega_note(
            "Install claude CLI",
            "macOS / Linux:\n  curl -fsSL https://claude.ai/install.sh | bash\n\n\
             Windows PowerShell:\n  irm https://claude.ai/install.ps1 | iex\n\n\
             Windows CMD:\n  curl -fsSL https://claude.ai/install.cmd -o install.cmd && install.cmd && del install.cmd\n\n\
             Then run 'omega init' again.",
        )?;
        init_style::omega_outro_cancel("Setup aborted")?;
        return Ok(());
    }

    // 3. Anthropic authentication (OAuth token for CLAUDE_CODE_OAUTH_TOKEN).
    let oauth_token = init_wizard::run_anthropic_auth()?;

    // 4. Telegram bot token.
    let bot_token: String = cliclack::input("Telegram bot token")
        .placeholder("Paste token from @BotFather (or Enter to skip)")
        .required(false)
        .default_input("")
        .interact()?;
    let bot_token = bot_token.trim().to_string();

    if bot_token.is_empty() {
        init_style::omega_info("Skipping Telegram — you can add it later in config.toml")?;
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
            init_style::omega_note(
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

    // 8. Optional tools.
    let omg_gog_installed = crate::init_google::is_omg_gog_installed();
    let google_hint = if omg_gog_installed {
        "Ask OMEGA to manage Gmail, Calendar, Drive, Docs... (installed)"
    } else {
        "Ask OMEGA to manage Gmail, Calendar, Drive, Docs..."
    };

    let hint = console::Style::new()
        .bold()
        .apply_to("Space to select, Enter to confirm");
    init_style::omega_info(&hint.to_string())?;

    let selected_tools: Vec<&str> = cliclack::multiselect("Optional tools — select to set up")
        .item("skip", "Skip", "Continue without setting up optional tools")
        .item("google", "Google Workspace", google_hint)
        .required(true)
        .interact()?;

    let selected_tools: Vec<&str> = if selected_tools.contains(&"skip") {
        vec![]
    } else {
        selected_tools
    };

    let google_email = if selected_tools.contains(&"google") {
        let ready = if omg_gog_installed {
            true
        } else {
            crate::init_google::install_omg_gog()?
        };
        if ready {
            crate::init_google::run_google_wizard()?
        } else {
            None
        }
    } else {
        None
    };

    // 9. Generate config.toml at ~/.omega/config.toml.
    let config_path_expanded = shellexpand("~/.omega/config.toml");
    let config_path = config_path_expanded.as_str();
    if Path::new(config_path).exists() {
        init_style::omega_warning(
            "~/.omega/config.toml already exists — skipping.\nDelete it and run 'omega init' again to regenerate.",
        )?;
    } else {
        let user_ids: Vec<i64> = user_id.into_iter().collect();
        let config = generate_config(
            &bot_token,
            &user_ids,
            whisper_api_key.as_deref(),
            whatsapp_enabled,
            google_email.as_deref(),
            oauth_token.as_deref(),
        );
        std::fs::write(config_path, config)?;
        init_style::omega_success("Generated ~/.omega/config.toml")?;
    }

    // 11. Offer service installation.
    let install_service: bool = cliclack::confirm("Install Omega as a system service?")
        .initial_value(true)
        .interact()?;

    let service_installed = if install_service {
        match service::install("~/.omega/config.toml") {
            Ok(()) => true,
            Err(e) => {
                init_style::omega_warning(&format!("Service install failed: {e}"))?;
                init_style::omega_info("You can install later with: omega service install")?;
                false
            }
        }
    } else {
        false
    };

    // 12. Next steps.
    let mut steps = String::from(
        "1. Review ~/.omega/config.toml\n2. Run: omega start\n3. Send a message to your bot",
    );
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
    init_style::omega_note("Next steps", &steps)?;

    init_style::omega_outro("Setup complete")?;
    init_style::typewrite("\n  enjoy OMEGA Ω!\n\n", 30);
    Ok(())
}

/// Menu-driven reconfiguration for an existing OMEGA installation.
///
/// Lets the user pick individual components to re-setup and updates
/// `~/.omega/config.toml` in-place without regenerating the whole file.
pub async fn run_setup() -> anyhow::Result<()> {
    let config_path = shellexpand("~/.omega/config.toml");
    if !Path::new(&config_path).exists() {
        init_style::omega_intro(LOGO, "omega setup")?;
        init_style::omega_warning("OMEGA is not installed. Run `omega init` first.")?;
        init_style::omega_outro_cancel("Nothing to configure")?;
        return Ok(());
    }

    loop {
        init_style::omega_intro(LOGO, "omega setup")?;

        // Detect which components are already configured.
        let cfg: toml::Table = std::fs::read_to_string(&config_path)
            .unwrap_or_default()
            .parse()
            .unwrap_or_default();
        let has = |path: &[&str]| -> bool {
            let mut val: &toml::Value = match cfg.get(path[0]) {
                Some(v) => v,
                None => return false,
            };
            for &key in &path[1..] {
                val = match val.get(key) {
                    Some(v) => v,
                    None => return false,
                };
            }
            match val {
                toml::Value::String(s) => !s.is_empty(),
                toml::Value::Boolean(b) => *b,
                _ => true,
            }
        };
        let check = |configured: bool, label: &str| -> String {
            if configured {
                format!("{label} (configured)")
            } else {
                label.to_string()
            }
        };
        let has_claude = has(&["provider", "claude-code", "oauth_token"]);
        let has_telegram = has(&["channel", "telegram", "bot_token"]);
        let has_whisper = has(&["channel", "telegram", "whisper_api_key"])
            || has(&["channel", "whatsapp", "whisper_api_key"]);
        let has_whatsapp = has(&["channel", "whatsapp", "enabled"]);
        let has_google = has(&["google", "account"]);
        let has_service = if cfg!(target_os = "macos") {
            Path::new(&shellexpand(
                "~/Library/LaunchAgents/com.omega-cortex.omega.plist",
            ))
            .exists()
        } else {
            Path::new(&shellexpand("~/.config/systemd/user/omega.service")).exists()
        };

        let hint = console::Style::new()
            .bold()
            .apply_to("Space to select, Enter to confirm");
        init_style::omega_info(&hint.to_string())?;

        let lbl_claude = check(has_claude, "Claude Auth");
        let lbl_telegram = check(has_telegram, "Telegram");
        let lbl_whisper = check(has_whisper, "Voice Transcription");
        let lbl_whatsapp = check(has_whatsapp, "WhatsApp");
        let lbl_google = check(has_google, "Google Workspace");
        let lbl_service = check(has_service, "System Service");

        let selected: Vec<&str> = cliclack::multiselect("Select components to reconfigure")
            .item("claude", &lbl_claude, "OAuth token for Claude Code")
            .item("telegram", &lbl_telegram, "Bot token and allowed users")
            .item("whisper", &lbl_whisper, "OpenAI Whisper API key")
            .item("whatsapp", &lbl_whatsapp, "Pair via QR code")
            .item("google", &lbl_google, "Gmail, Calendar, Drive...")
            .item("service", &lbl_service, "Install or reinstall the service")
            .item("exit", "Exit", "Return to terminal")
            .interact()?;

        if selected.contains(&"exit") {
            init_style::omega_outro("Done")?;
            return Ok(());
        }

        let mut updates: Vec<(&str, &str, String)> = Vec::new();
        let mut changed: Vec<&str> = Vec::new();

        // ── Claude Auth ──────────────────────────────────────────────────
        if selected.contains(&"claude") {
            if let Some(token) = init_wizard::run_anthropic_auth()? {
                updates.push(("provider", "claude-code.oauth_token", token));
                changed.push("Claude Auth");
            }
        }

        // ── Telegram ─────────────────────────────────────────────────────
        if selected.contains(&"telegram") {
            let bot_token: String = cliclack::input("Telegram bot token")
                .placeholder("Paste token from @BotFather (or Enter to skip)")
                .required(false)
                .default_input("")
                .interact()?;
            let bot_token = bot_token.trim().to_string();

            if !bot_token.is_empty() {
                updates.push(("channel", "telegram.bot_token", bot_token));
                updates.push(("channel", "telegram.enabled", "true".to_string()));

                let id_str: String = cliclack::input("Your Telegram user ID")
                    .placeholder("Send /start to @userinfobot (blank = allow all)")
                    .required(false)
                    .default_input("")
                    .interact()?;
                if let Ok(id) = id_str.parse::<i64>() {
                    updates.push(("channel", "telegram.allowed_users", format!("[{id}]")));
                }
                changed.push("Telegram");
            } else {
                init_style::omega_info("Skipping Telegram — no changes")?;
            }
        }

        // ── Whisper ──────────────────────────────────────────────────────
        if selected.contains(&"whisper") {
            let key: String = cliclack::input("OpenAI API key (for Whisper)")
                .placeholder("sk-... (Enter to skip)")
                .required(false)
                .default_input("")
                .interact()?;
            let key = key.trim().to_string();
            if !key.is_empty() {
                updates.push(("channel", "telegram.whisper_api_key", key));
                changed.push("Voice Transcription");
            }
        }

        // ── WhatsApp ─────────────────────────────────────────────────────
        if selected.contains(&"whatsapp") {
            let paired = init_wizard::run_whatsapp_setup().await?;
            if paired {
                updates.push(("channel", "whatsapp.enabled", "true".to_string()));
                changed.push("WhatsApp");
            }
        }

        // ── Google Workspace ─────────────────────────────────────────────
        if selected.contains(&"google") {
            let omg_gog_installed = crate::init_google::is_omg_gog_installed();
            let ready = if omg_gog_installed {
                true
            } else {
                crate::init_google::install_omg_gog()?
            };
            if ready {
                if let Some(email) = crate::init_google::run_google_wizard()? {
                    updates.push(("google", "account", email));
                    changed.push("Google Workspace");
                }
            }
        }

        // ── Apply config updates ─────────────────────────────────────────
        if !updates.is_empty() {
            update_config(&config_path, &updates)?;
            init_style::omega_success(&format!(
                "Updated ~/.omega/config.toml — {}",
                changed.join(", ")
            ))?;
        }

        // ── System Service ───────────────────────────────────────────────
        if selected.contains(&"service") {
            match service::install("~/.omega/config.toml") {
                Ok(()) => {
                    init_style::omega_success("System service installed")?;
                }
                Err(e) => {
                    init_style::omega_warning(&format!("Service install failed: {e}"))?;
                }
            }
        }
    }
}

/// Update `config.toml` in-place by setting keys in nested tables.
///
/// Each entry in `updates` is `(section, dotted_key, value)`:
/// - `("provider", "claude-code.oauth_token", "sk-...")` sets
///   `[provider.claude-code] oauth_token = "sk-..."`
/// - `("channel", "telegram.bot_token", "123:ABC")` sets
///   `[channel.telegram] bot_token = "123:ABC"`
/// - `("google", "account", "me@gmail.com")` sets
///   `[google] account = "me@gmail.com"`
pub fn update_config(path: &str, updates: &[(&str, &str, String)]) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(path)?;
    let mut doc: toml::Table = content
        .parse()
        .map_err(|e| anyhow::anyhow!("failed to parse config.toml: {e}"))?;

    for (section, dotted_key, value) in updates {
        // Split dotted_key: "telegram.bot_token" → ["telegram", "bot_token"]
        let parts: Vec<&str> = dotted_key.split('.').collect();

        // Navigate/create the table path: section → sub-tables
        let mut table = doc
            .entry(section.to_string())
            .or_insert_with(|| toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .ok_or_else(|| anyhow::anyhow!("[{section}] is not a table"))?;

        for &part in &parts[..parts.len().saturating_sub(1)] {
            table = table
                .entry(part)
                .or_insert_with(|| toml::Value::Table(toml::Table::new()))
                .as_table_mut()
                .ok_or_else(|| anyhow::anyhow!("[{section}.{part}] is not a table"))?;
        }

        let leaf = parts.last().ok_or_else(|| anyhow::anyhow!("empty key"))?;

        // Parse value: try bool, then integer array, then string.
        let toml_value = if value == "true" || value == "false" {
            toml::Value::Boolean(value == "true")
        } else if value.starts_with('[') && value.ends_with(']') {
            // Parse as TOML array value.
            let parsed: toml::Value = format!("v = {value}")
                .parse::<toml::Table>()
                .map(|t| t["v"].clone())
                .unwrap_or_else(|_| toml::Value::String(value.clone()));
            parsed
        } else {
            toml::Value::String(value.clone())
        };

        table.insert(leaf.to_string(), toml_value);
    }

    let output = toml::to_string_pretty(&doc)?;
    std::fs::write(path, output)?;
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
    init_style::omega_step("OMEGA -- non-interactive init")?;

    // 1. Validate inputs.
    let user_ids = parse_allowed_users(allowed_users_csv)?;

    // 2. Create data directory.
    let data_dir = shellexpand("~/.omega");
    std::fs::create_dir_all(&data_dir)?;
    init_style::omega_success(&format!("Data directory: {data_dir}"))?;

    // 3. Check claude CLI (warn if missing, non-fatal).
    let claude_ok = std::process::Command::new("claude")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if claude_ok {
        init_style::omega_success("claude CLI: found")?;
    } else {
        init_style::omega_warning(
            "claude CLI not found -- install with: curl -fsSL https://claude.ai/install.sh | bash (or see omega init for all platforms)",
        )?;
    }

    // 4. Claude OAuth token — stored in config.toml, injected as env var by the provider.
    if claude_setup_token.is_some() {
        init_style::omega_success("Claude OAuth token: will be saved to config.toml")?;
    }

    // 5. Google credentials setup (non-interactive part only).
    if let Some(cred_path) = google_credentials {
        let expanded = shellexpand(cred_path);
        if !Path::new(&expanded).exists() {
            init_style::omega_warning(&format!("Google credentials file not found: {expanded}"))?;
        } else {
            let result = std::process::Command::new("omg-gog")
                .args(["auth", "credentials", &expanded])
                .output();
            match result {
                Ok(output) if output.status.success() => {
                    init_style::omega_success("Google credentials: registered")?;
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    init_style::omega_warning(&format!(
                        "omg-gog auth credentials failed: {stderr}"
                    ))?;
                }
                Err(e) => {
                    init_style::omega_warning(&format!("could not run omg-gog: {e}"))?;
                }
            }
        }
        if let Some(email) = google_email {
            init_style::omega_note(
                "Google OAuth",
                &format!("Complete post-deployment:\nomg-gog auth add {email} --services gmail,calendar,drive,contacts,docs,sheets"),
            )?;
        }
    }

    // 6. Bail if config.toml already exists.
    let config_path_expanded = shellexpand("~/.omega/config.toml");
    let config_path = config_path_expanded.as_str();
    if Path::new(config_path).exists() {
        anyhow::bail!(
            "~/.omega/config.toml already exists — delete it first or use a different directory"
        );
    }

    // 7. Generate and write config.toml at ~/.omega/.
    let config = generate_config(
        telegram_token,
        &user_ids,
        whisper_key,
        false, // WhatsApp handled post-deployment
        google_email,
        claude_setup_token,
    );
    std::fs::write(config_path, &config)?;
    init_style::omega_success("Generated: ~/.omega/config.toml")?;

    // 8. Deploy bundled prompts and skills.
    omega_core::config::install_bundled_prompts("~/.omega");
    init_style::omega_success("Deployed: bundled prompts")?;

    omega_skills::install_bundled_skills("~/.omega");
    init_style::omega_success("Deployed: bundled skills")?;

    // 9. Create workspace directory.
    let ws = std::path::PathBuf::from(&data_dir).join("workspace");
    std::fs::create_dir_all(&ws)?;
    init_style::omega_success(&format!("Workspace: {}", ws.display()))?;

    // 10. Install system service (non-interactive).
    match service::install_quiet("~/.omega/config.toml") {
        Ok(()) => init_style::omega_success("Service: installed and activated")?,
        Err(e) => init_style::omega_warning(&format!(
            "Service install failed: {e}\nInstall later with: omega service install"
        ))?,
    }

    // 11. Summary.
    let mut summary = "Config: ~/.omega/config.toml\nStart: omega start".to_string();
    if google_credentials.is_some() && google_email.is_some() {
        summary.push_str(&format!(
            "\nGoogle OAuth: complete post-deployment with:\n  omg-gog auth add {} --services gmail,calendar,drive,contacts,docs,sheets",
            google_email.unwrap_or("your@email.com")
        ));
    }
    init_style::omega_note("OMEGA -- init complete!", &summary)?;

    Ok(())
}

/// Generate config.toml content from wizard inputs (pure function for testability).
pub fn generate_config(
    bot_token: &str,
    user_ids: &[i64],
    whisper_api_key: Option<&str>,
    whatsapp_enabled: bool,
    google_email: Option<&str>,
    oauth_token: Option<&str>,
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

    let oauth_line = match oauth_token {
        Some(token) if !token.is_empty() => format!("oauth_token = \"{token}\""),
        _ => "# oauth_token = \"\"  # Paste token from `claude setup-token`".to_string(),
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
{oauth_line}

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
            Some("sk-ant-oat01-test"),
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
        assert!(config.contains("oauth_token = \"sk-ant-oat01-test\""));
        assert!(!config.contains("[sandbox]"), "no sandbox section");
    }

    #[test]
    fn test_generate_config_minimal() {
        let config = generate_config("", &[], None, false, None, None);
        assert!(config.contains("bot_token = \"\""));
        assert!(config.contains("allowed_users = []"));
        assert!(config.contains("[channel.telegram]\nenabled = false"));
        assert!(config.contains("[channel.whatsapp]\nenabled = false"));
        assert!(config.contains("# oauth_token"));
        assert!(!config.contains("[google]"));
        assert!(!config.contains("[sandbox]"));
    }

    #[test]
    fn test_generate_config_telegram_only() {
        let config = generate_config("tok:EN", &[999], None, false, None, None);
        assert!(config.contains("bot_token = \"tok:EN\""));
        assert!(config.contains("allowed_users = [999]"));
        assert!(config.contains("[channel.telegram]\nenabled = true"));
        assert!(config.contains("[channel.whatsapp]\nenabled = false"));
        assert!(!config.contains("[google]"));
    }

    #[test]
    fn test_generate_config_google_only() {
        let config = generate_config("", &[], None, false, Some("test@example.com"), None);
        assert!(config.contains("[google]\naccount = \"test@example.com\""));
        assert!(config.contains("[channel.telegram]\nenabled = false"));
    }

    #[test]
    fn test_generate_config_whatsapp_only() {
        let config = generate_config("", &[], None, true, None, None);
        assert!(config.contains("[channel.whatsapp]\nenabled = true"));
        assert!(config.contains("[channel.telegram]\nenabled = false"));
        assert!(!config.contains("[google]"));
    }

    #[test]
    fn test_generate_config_with_whisper() {
        let config = generate_config("tok:EN", &[42], Some("sk-abc"), false, None, None);
        assert!(config.contains("whisper_api_key = \"sk-abc\""));
    }

    #[test]
    fn test_generate_config_without_whisper() {
        let config = generate_config("tok:EN", &[42], None, false, None, None);
        assert!(config.contains("# whisper_api_key"));
        assert!(config.contains("OPENAI_API_KEY"));
    }

    #[test]
    fn test_generate_config_multiple_users() {
        let config = generate_config("tok:EN", &[111, 222, 333], None, false, None, None);
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

    #[test]
    fn test_update_config_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let initial = generate_config("old:TOKEN", &[42], None, false, None, None);
        std::fs::write(&path, &initial).unwrap();

        let path_str = path.to_str().unwrap();
        update_config(
            path_str,
            &[
                ("channel", "telegram.bot_token", "new:TOKEN".to_string()),
                ("channel", "telegram.enabled", "true".to_string()),
                ("channel", "whatsapp.enabled", "true".to_string()),
                (
                    "provider",
                    "claude-code.oauth_token",
                    "sk-ant-new".to_string(),
                ),
                ("google", "account", "me@gmail.com".to_string()),
            ],
        )
        .unwrap();

        let result = std::fs::read_to_string(&path).unwrap();
        let doc: toml::Table = result.parse().unwrap();

        // Verify telegram updates.
        let tg = doc["channel"]["telegram"].as_table().unwrap();
        assert_eq!(tg["bot_token"].as_str().unwrap(), "new:TOKEN");
        assert!(tg["enabled"].as_bool().unwrap());

        // Verify whatsapp update.
        let wa = doc["channel"]["whatsapp"].as_table().unwrap();
        assert!(wa["enabled"].as_bool().unwrap());

        // Verify provider update.
        let cc = doc["provider"]["claude-code"].as_table().unwrap();
        assert_eq!(cc["oauth_token"].as_str().unwrap(), "sk-ant-new");

        // Verify google section was created.
        assert_eq!(doc["google"]["account"].as_str().unwrap(), "me@gmail.com");

        // Verify existing fields were preserved.
        assert_eq!(doc["omega"]["name"].as_str().unwrap(), "OMEGA Ω");
        assert_eq!(doc["memory"]["backend"].as_str().unwrap(), "sqlite");
    }

    #[test]
    fn test_update_config_creates_missing_section() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[omega]\nname = \"test\"\n").unwrap();

        let path_str = path.to_str().unwrap();
        update_config(path_str, &[("google", "account", "a@b.com".to_string())]).unwrap();

        let result = std::fs::read_to_string(&path).unwrap();
        let doc: toml::Table = result.parse().unwrap();
        assert_eq!(doc["google"]["account"].as_str().unwrap(), "a@b.com");
        assert_eq!(doc["omega"]["name"].as_str().unwrap(), "test");
    }

    #[test]
    fn test_update_config_array_value() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[channel.telegram]\nallowed_users = []\n").unwrap();

        let path_str = path.to_str().unwrap();
        update_config(
            path_str,
            &[(
                "channel",
                "telegram.allowed_users",
                "[123, 456]".to_string(),
            )],
        )
        .unwrap();

        let result = std::fs::read_to_string(&path).unwrap();
        let doc: toml::Table = result.parse().unwrap();
        let users = doc["channel"]["telegram"]["allowed_users"]
            .as_array()
            .unwrap();
        assert_eq!(users.len(), 2);
        assert_eq!(users[0].as_integer().unwrap(), 123);
        assert_eq!(users[1].as_integer().unwrap(), 456);
    }
}
