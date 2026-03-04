//! Interactive-only helpers for the init wizard (auth, WhatsApp).
//! Google Workspace setup lives in `init_google.rs`.

use crate::init_style;
use omega_channels::whatsapp;
use omega_core::shellexpand;
use std::path::Path;

/// Check Claude CLI authentication via `claude auth status`.
///
/// Returns `(logged_in, Option<email>)` parsed from the JSON output.
fn check_claude_auth_status() -> (bool, Option<String>) {
    let output = std::process::Command::new("claude")
        .args(["auth", "status"])
        .output();

    let Ok(output) = output else {
        return (false, None);
    };

    if !output.status.success() {
        return (false, None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let Ok(value) = serde_json::from_str::<serde_json::Value>(stdout.trim()) else {
        return (false, None);
    };

    let logged_in = value
        .get("loggedIn")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let email = value
        .get("email")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    (logged_in, email)
}

/// Check Claude CLI authentication status and inform the user.
///
/// If logged in, shows the email and continues. If not, instructs the user
/// to run `claude login` before re-running `omega init`.
pub(crate) fn run_anthropic_auth() -> anyhow::Result<()> {
    let spinner = cliclack::spinner();
    spinner.start("Checking Claude Code authentication...");
    let (logged_in, email) = check_claude_auth_status();

    if logged_in {
        let msg = if let Some(ref email) = email {
            format!("Claude Code — logged in as {email}")
        } else {
            "Claude Code — authenticated".to_string()
        };
        spinner.stop(msg);
    } else {
        spinner.error("Claude Code — not logged in");
        init_style::omega_warning(
            "Run `claude login` to authenticate, then run `omega init` again.",
        )?;
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
pub(crate) async fn run_whatsapp_setup() -> anyhow::Result<bool> {
    // If already paired, skip the QR flow.
    if whatsapp_already_paired() {
        init_style::omega_success("WhatsApp — already paired")?;
        return Ok(true);
    }

    let connect: bool = cliclack::confirm("Connect WhatsApp?")
        .initial_value(false)
        .interact()?;

    if !connect {
        return Ok(false);
    }

    init_style::omega_step("Starting WhatsApp pairing...")?;
    init_style::omega_info("Open WhatsApp on your phone > Linked Devices > Link a Device")?;

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
        init_style::omega_note("Scan this QR code with WhatsApp", &qr_text)?;

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
            init_style::omega_warning("You can try again later with /whatsapp.")?;
            Ok(false)
        }
        Err(e) => {
            init_style::omega_error(&format!("{e} — you can try again later with /whatsapp."))?;
            Ok(false)
        }
    }
}
