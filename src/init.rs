//! Init wizard — interactive setup for new users with cliclack styled prompts.

use crate::service;
use omega_channels::whatsapp;
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
    run_anthropic_auth()?;

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

    // 6. WhatsApp setup.
    let whatsapp_enabled = run_whatsapp_setup().await?;

    // 7. Google Workspace setup.
    let google_email = run_google_setup()?;

    // 8. Sandbox mode.
    let sandbox_mode: &str = cliclack::select("Sandbox mode")
        .item(
            "sandbox",
            "Sandbox (Recommended)",
            "Workspace only — no host filesystem access",
        )
        .item(
            "rx",
            "Read-only",
            "Read & execute on host, writes only in workspace",
        )
        .item(
            "rwx",
            "Full access",
            "Unrestricted host access (power users)",
        )
        .interact()?;

    // 9. Generate config.toml.
    let config_path = "config.toml";
    if Path::new(config_path).exists() {
        cliclack::log::warning(
            "config.toml already exists — skipping.\nDelete it and run 'omega init' again to regenerate.",
        )?;
    } else {
        let config = generate_config(
            &bot_token,
            user_id,
            whatsapp_enabled,
            google_email.as_deref(),
            sandbox_mode,
        );
        std::fs::write(config_path, config)?;
        cliclack::log::success("Generated config.toml")?;
    }

    // 10. Offer service installation.
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

    // 11. Next steps.
    let mut steps =
        String::from("1. Review config.toml\n2. Run: omega start\n3. Send a message to your bot");
    if whatsapp_enabled {
        steps.push_str("\n4. WhatsApp is linked and ready!");
    }
    if google_email.is_some() {
        steps.push_str("\n★ Google Workspace is connected!");
    }
    if service_installed {
        steps.push_str("\n★ System service installed — Ω OMEGA starts on login!");
    } else if !install_service {
        steps.push_str("\nTip: Run `omega service install` to auto-start on login");
    }
    cliclack::note("Next steps", &steps)?;

    cliclack::outro("Setup complete — enjoy Ω OMEGA!")?;
    Ok(())
}

/// Generate config.toml content from wizard inputs (pure function for testability).
pub fn generate_config(
    bot_token: &str,
    user_id: Option<i64>,
    whatsapp_enabled: bool,
    google_email: Option<&str>,
    sandbox_mode: &str,
) -> String {
    let allowed_users = match user_id {
        Some(id) => format!("[{id}]"),
        None => "[]".to_string(),
    };
    let telegram_enabled = if bot_token.is_empty() {
        "false"
    } else {
        "true"
    };
    let wa_enabled = if whatsapp_enabled { "true" } else { "false" };

    let mut config = format!(
        r#"[omega]
name = "Ω OMEGA"
data_dir = "~/.omega"
log_level = "info"

[auth]
enabled = true

[provider]
default = "claude-code"

[provider.claude-code]
enabled = true
max_turns = 10
allowed_tools = ["Bash", "Read", "Write", "Edit"]

[channel.telegram]
enabled = {telegram_enabled}
bot_token = "{bot_token}"
allowed_users = {allowed_users}

[channel.whatsapp]
enabled = {wa_enabled}
allowed_users = []

[memory]
backend = "sqlite"
db_path = "~/.omega/memory.db"
max_context_messages = 50

[sandbox]
mode = "{sandbox_mode}"
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

/// Run Anthropic authentication setup.
///
/// Offers the user a choice between "already authenticated" and pasting a setup-token.
fn run_anthropic_auth() -> anyhow::Result<()> {
    let auth_method: &str = cliclack::select("Anthropic auth method")
        .item(
            "authenticated",
            "Already authenticated (Recommended)",
            "Claude CLI is already logged in",
        )
        .item(
            "setup-token",
            "Paste setup-token",
            "Run `claude setup-token` elsewhere, then paste the token here",
        )
        .interact()?;

    if auth_method == "setup-token" {
        cliclack::note(
            "Anthropic setup-token",
            "Run `claude setup-token` in your terminal.\nThen paste the generated token below.",
        )?;

        let token: String = cliclack::input("Paste Anthropic setup-token")
            .placeholder("Paste the token here")
            .validate(|input: &String| {
                if input.trim().is_empty() {
                    return Err("Token is required");
                }
                Ok(())
            })
            .interact()?;

        let spinner = cliclack::spinner();
        spinner.start("Applying setup-token...");

        let result = std::process::Command::new("claude")
            .args(["setup-token", token.trim()])
            .output();

        match result {
            Ok(output) if output.status.success() => {
                spinner.stop("Anthropic authentication — configured");
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                spinner.error(format!("setup-token failed: {stderr}"));
                cliclack::log::warning("You can authenticate later with: claude setup-token")?;
            }
            Err(e) => {
                spinner.error(format!("Failed to run claude: {e}"));
                cliclack::log::warning("You can authenticate later with: claude setup-token")?;
            }
        }
    } else {
        cliclack::log::success("Anthropic authentication — already configured")?;
    }

    Ok(())
}

/// Check if a WhatsApp session already exists.
fn whatsapp_already_paired() -> bool {
    let dir = shellexpand("~/.omega/whatsapp_session");
    Path::new(&dir).join("whatsapp.db").exists()
}

/// Run WhatsApp pairing as part of the init wizard.
///
/// Returns `true` if WhatsApp was successfully paired.
async fn run_whatsapp_setup() -> anyhow::Result<bool> {
    // If already paired, skip the QR flow.
    if whatsapp_already_paired() {
        cliclack::log::success("WhatsApp — already paired")?;
        return Ok(true);
    }

    let connect: bool = cliclack::confirm("Connect WhatsApp?")
        .initial_value(false)
        .interact()?;

    if !connect {
        return Ok(false);
    }

    cliclack::log::step("Starting WhatsApp pairing...")?;
    cliclack::log::info("Open WhatsApp on your phone > Linked Devices > Link a Device")?;

    let result = async {
        let (mut qr_rx, mut done_rx) = whatsapp::start_pairing("~/.omega").await?;

        // Wait for the first QR code.
        let qr_data = tokio::time::timeout(std::time::Duration::from_secs(30), qr_rx.recv())
            .await
            .map_err(|_| anyhow::anyhow!("timed out waiting for QR code"))?
            .ok_or_else(|| anyhow::anyhow!("QR channel closed"))?;

        // Render QR in terminal.
        let qr_text = whatsapp::generate_qr_terminal(&qr_data)?;
        // Display QR code inside a cliclack note.
        cliclack::note("Scan this QR code with WhatsApp", &qr_text)?;

        let spinner = cliclack::spinner();
        spinner.start("Waiting for scan...");

        // Wait for pairing confirmation.
        let paired = tokio::time::timeout(std::time::Duration::from_secs(60), done_rx.recv())
            .await
            .map_err(|_| anyhow::anyhow!("pairing timed out"))?
            .unwrap_or(false);

        if paired {
            spinner.stop("WhatsApp linked successfully");
        } else {
            spinner.error("Pairing did not complete");
        }

        Ok::<bool, anyhow::Error>(paired)
    }
    .await;

    match result {
        Ok(true) => Ok(true),
        Ok(false) => {
            cliclack::log::warning("You can try again later with /whatsapp.")?;
            Ok(false)
        }
        Err(e) => {
            cliclack::log::error(format!("{e} — you can try again later with /whatsapp."))?;
            Ok(false)
        }
    }
}

/// Run Google Workspace setup using the `gog` CLI tool.
///
/// Returns `Some(email)` if Google was successfully connected.
fn run_google_setup() -> anyhow::Result<Option<String>> {
    // Check if gog CLI is available.
    let gog_ok = std::process::Command::new("gog")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !gog_ok {
        // gog not installed — skip silently (don't even ask).
        return Ok(None);
    }

    let setup: bool = cliclack::confirm("Set up Google Workspace? (Gmail, Calendar, Drive)")
        .initial_value(false)
        .interact()?;

    if !setup {
        return Ok(None);
    }

    // Show setup instructions.
    cliclack::note(
        "Google Workspace Setup",
        "1. Go to console.cloud.google.com\n\
         2. Create a project (or use existing)\n\
         3. Enable: Gmail API, Calendar API, Drive API\n\
         4. Go to OAuth consent screen → Audience → Publish app\n\
         5. Go to Credentials → Create OAuth Client ID → Desktop app\n\
         6. Download the JSON file",
    )?;

    // Ask for credentials file path.
    let cred_path: String = cliclack::input("Path to client_secret.json")
        .placeholder("~/Downloads/client_secret_xxxxx.json")
        .validate(|input: &String| {
            if input.is_empty() {
                return Err("Path is required");
            }
            let expanded = shellexpand(input);
            if !Path::new(&expanded).exists() {
                return Err("File not found");
            }
            Ok(())
        })
        .interact()?;

    let expanded_cred = shellexpand(&cred_path);

    // Run: gog auth credentials <path>
    let spinner = cliclack::spinner();
    spinner.start("Running: gog auth credentials ...");
    let cred_result = std::process::Command::new("gog")
        .args(["auth", "credentials", &expanded_cred])
        .output();

    match cred_result {
        Ok(output) if output.status.success() => {
            spinner.stop("Credentials registered");
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            spinner.error(format!("gog auth credentials failed: {stderr}"));
            cliclack::log::warning("Skipping Google Workspace setup.")?;
            return Ok(None);
        }
        Err(e) => {
            spinner.error(format!("Failed to run gog: {e}"));
            cliclack::log::warning("Skipping Google Workspace setup.")?;
            return Ok(None);
        }
    }

    // Ask for Gmail address.
    let email: String = cliclack::input("Your Gmail address")
        .placeholder("you@gmail.com")
        .validate(|input: &String| {
            if input.is_empty() || !input.contains('@') {
                return Err("Please enter a valid email address");
            }
            Ok(())
        })
        .interact()?;

    // Show OAuth troubleshooting tips before starting the flow.
    cliclack::note(
        "OAuth Tips",
        "A browser will open for Google sign-in.\n\
         • Click 'Advanced' → 'Go to gog (unsafe)' → Allow\n\
         • If you see 'Something went wrong', try in an incognito window:\n\
           gog auth add <email> --services gmail,calendar,drive,contacts,docs,sheets\n\
         • If 'Access blocked: not verified', go to OAuth consent screen →\n\
           Audience → Publish app (or add yourself as a test user)",
    )?;

    // Run: gog auth add <email> --services gmail,calendar,drive,contacts,docs,sheets
    let spinner = cliclack::spinner();
    spinner.start("Waiting for OAuth approval in browser...");
    let auth_result = std::process::Command::new("gog")
        .args([
            "auth",
            "add",
            &email,
            "--services",
            "gmail,calendar,drive,contacts,docs,sheets",
        ])
        .output();

    match auth_result {
        Ok(output) if output.status.success() => {
            spinner.stop("OAuth approved");
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            spinner.error(format!("gog auth add failed: {stderr}"));
            cliclack::log::warning(
                "If your browser showed an error, try in an incognito window:\n\
                 gog auth add <email> --services gmail,calendar,drive,contacts,docs,sheets",
            )?;
            return Ok(None);
        }
        Err(e) => {
            spinner.error(format!("Failed to run gog: {e}"));
            cliclack::log::warning("Google Workspace setup incomplete.")?;
            return Ok(None);
        }
    }

    // Verify with gog auth list.
    let verify = std::process::Command::new("gog")
        .args(["auth", "list"])
        .output();

    if let Ok(output) = verify {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains(&email) {
            cliclack::log::success("Google Workspace connected!")?;
            return Ok(Some(email));
        }
    }

    // Verification couldn't confirm, but auth might still have worked.
    cliclack::log::warning("Could not verify Google auth — check manually with 'gog auth list'.")?;
    Ok(Some(email))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_config_full() {
        let config = generate_config("123:ABC", Some(42), true, Some("me@gmail.com"), "sandbox");
        assert!(config.contains("bot_token = \"123:ABC\""));
        assert!(config.contains("allowed_users = [42]"));
        assert!(
            config.contains("enabled = true"),
            "telegram should be enabled"
        );
        assert!(config.contains("[channel.whatsapp]\nenabled = true"));
        assert!(config.contains("[google]\naccount = \"me@gmail.com\""));
        assert!(config.contains("[sandbox]\nmode = \"sandbox\""));
    }

    #[test]
    fn test_generate_config_minimal() {
        let config = generate_config("", None, false, None, "sandbox");
        assert!(config.contains("bot_token = \"\""));
        assert!(config.contains("allowed_users = []"));
        assert!(config.contains("[channel.telegram]\nenabled = false"));
        assert!(config.contains("[channel.whatsapp]\nenabled = false"));
        assert!(!config.contains("[google]"));
        assert!(config.contains("mode = \"sandbox\""));
    }

    #[test]
    fn test_generate_config_telegram_only() {
        let config = generate_config("tok:EN", Some(999), false, None, "sandbox");
        assert!(config.contains("bot_token = \"tok:EN\""));
        assert!(config.contains("allowed_users = [999]"));
        assert!(config.contains("[channel.telegram]\nenabled = true"));
        assert!(config.contains("[channel.whatsapp]\nenabled = false"));
        assert!(!config.contains("[google]"));
    }

    #[test]
    fn test_generate_config_google_only() {
        let config = generate_config("", None, false, Some("test@example.com"), "sandbox");
        assert!(config.contains("[google]\naccount = \"test@example.com\""));
        assert!(config.contains("[channel.telegram]\nenabled = false"));
    }

    #[test]
    fn test_generate_config_whatsapp_only() {
        let config = generate_config("", None, true, None, "sandbox");
        assert!(config.contains("[channel.whatsapp]\nenabled = true"));
        assert!(config.contains("[channel.telegram]\nenabled = false"));
        assert!(!config.contains("[google]"));
    }

    #[test]
    fn test_generate_config_sandbox_modes() {
        let rx = generate_config("", None, false, None, "rx");
        assert!(rx.contains("mode = \"rx\""));

        let rwx = generate_config("", None, false, None, "rwx");
        assert!(rwx.contains("mode = \"rwx\""));
    }
}
