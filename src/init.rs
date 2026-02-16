//! Init wizard — interactive 2-minute setup for new users.

use omega_channels::whatsapp;
use omega_core::shellexpand;
use std::io::{self, BufRead, Write};
use std::path::Path;

/// Run the interactive init wizard.
pub fn run() -> anyhow::Result<()> {
    println!();
    println!("  Omega — Setup Wizard");
    println!("  ====================");
    println!();

    // 1. Create data directory.
    let data_dir = shellexpand("~/.omega");
    if !Path::new(&data_dir).exists() {
        std::fs::create_dir_all(&data_dir)?;
        println!("  Created {data_dir}");
    } else {
        println!("  {data_dir} already exists");
    }

    // 2. Check claude CLI.
    print!("  Checking claude CLI... ");
    io::stdout().flush()?;
    let claude_ok = std::process::Command::new("claude")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if claude_ok {
        println!("found");
    } else {
        println!("NOT FOUND");
        println!();
        println!("  Install claude CLI first:");
        println!("    npm install -g @anthropic-ai/claude-code");
        println!();
        println!("  Then run 'omega init' again.");
        return Ok(());
    }

    // 3. Telegram bot token.
    println!();
    println!("  Telegram Bot Setup");
    println!("  ------------------");
    println!("  Create a bot with @BotFather on Telegram, then paste the token.");
    println!();
    let bot_token = prompt("  Bot token: ")?;
    if bot_token.is_empty() {
        println!("  Skipping Telegram setup.");
        println!("  You can add it later in config.toml.");
    }

    // 4. User ID (optional).
    let user_id = if !bot_token.is_empty() {
        println!();
        println!("  Your Telegram user ID (send /start to @userinfobot to find it).");
        println!("  Leave blank to allow all users.");
        let id = prompt("  User ID: ")?;
        id.parse::<i64>().ok()
    } else {
        None
    };

    // 5. WhatsApp setup.
    let whatsapp_enabled = run_whatsapp_setup()?;

    // 6. Generate config.toml.
    let config_path = "config.toml";
    if Path::new(config_path).exists() {
        println!();
        println!("  config.toml already exists — skipping generation.");
        println!("  Delete it and run 'omega init' again to regenerate.");
    } else {
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

        let config = format!(
            r#"[omega]
name = "Omega"
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
"#
        );

        std::fs::write(config_path, config)?;
        println!();
        println!("  Generated config.toml");
    }

    // 7. Summary and next steps.
    println!();
    println!("  Setup Complete");
    println!("  ==============");
    println!();
    println!("  Next steps:");
    println!("    1. Review config.toml");
    println!("    2. Run: omega start");
    println!("    3. Send a message to your bot on Telegram");
    if whatsapp_enabled {
        println!("    4. WhatsApp is linked and ready!");
    }
    println!();

    Ok(())
}

/// Run WhatsApp pairing as part of the init wizard.
///
/// Returns `true` if WhatsApp was successfully paired.
fn run_whatsapp_setup() -> anyhow::Result<bool> {
    println!();
    println!("  WhatsApp Setup");
    println!("  --------------");
    let answer = prompt("  Would you like to connect WhatsApp? [y/N]: ")?;
    if !answer.eq_ignore_ascii_case("y") {
        println!("  Skipping WhatsApp setup.");
        return Ok(false);
    }

    println!();
    println!("  Starting WhatsApp pairing...");
    println!("  Open WhatsApp on your phone > Linked Devices > Link a Device");
    println!();

    // Use a small tokio runtime for the pairing flow.
    let rt = tokio::runtime::Runtime::new()?;
    let result = rt.block_on(async {
        let (mut qr_rx, mut done_rx) = whatsapp::start_pairing("~/.omega").await?;

        // Wait for the first QR code.
        let qr_data = tokio::time::timeout(std::time::Duration::from_secs(30), qr_rx.recv())
            .await
            .map_err(|_| anyhow::anyhow!("timed out waiting for QR code"))?
            .ok_or_else(|| anyhow::anyhow!("QR channel closed"))?;

        // Render QR in terminal.
        let qr_text = whatsapp::generate_qr_terminal(&qr_data)?;
        println!("  Scan this QR code with WhatsApp:\n");
        for line in qr_text.lines() {
            println!("  {line}");
        }
        println!();
        print!("  Waiting for scan... ");
        io::stdout().flush()?;

        // Wait for pairing confirmation.
        let paired = tokio::time::timeout(std::time::Duration::from_secs(60), done_rx.recv())
            .await
            .map_err(|_| anyhow::anyhow!("pairing timed out"))?
            .unwrap_or(false);

        Ok::<bool, anyhow::Error>(paired)
    });

    match result {
        Ok(true) => {
            println!("Connected!");
            println!("  WhatsApp linked successfully.");
            Ok(true)
        }
        Ok(false) => {
            println!("Failed.");
            println!("  Pairing did not complete. You can try again later with /whatsapp.");
            Ok(false)
        }
        Err(e) => {
            println!("Error: {e}");
            println!("  You can try again later with /whatsapp.");
            Ok(false)
        }
    }
}

/// Read a line from stdin with a prompt.
fn prompt(msg: &str) -> anyhow::Result<String> {
    print!("{msg}");
    io::stdout().flush()?;
    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;
    Ok(line.trim().to_string())
}
