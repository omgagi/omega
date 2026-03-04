//! Interactive-only helpers for the init wizard (auth, WhatsApp).
//! Google Workspace setup lives in `init_google.rs`.

use crate::init_style;
use omega_channels::whatsapp;
use omega_core::shellexpand;
use std::path::Path;

/// Probe whether the Claude CLI has valid authentication (15-second timeout).
///
/// Runs a minimal `claude -p` invocation. If the CLI has credentials the
/// command succeeds (exit 0); otherwise it fails fast before any network call.
fn is_claude_authenticated() -> bool {
    let child = std::process::Command::new("claude")
        .args(["-p", "ok", "--output-format", "json", "--max-turns", "1"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();

    let Ok(child) = child else {
        return false;
    };

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut child = child;
        let _ = tx.send(child.wait());
    });

    match rx.recv_timeout(std::time::Duration::from_secs(15)) {
        Ok(Ok(status)) => status.success(),
        _ => false,
    }
}

/// Validate an OAuth token by running `claude -p "ok"` with the token
/// as `CLAUDE_CODE_OAUTH_TOKEN` env var (15-second timeout).
fn validate_oauth_token(token: &str) -> bool {
    let child = std::process::Command::new("claude")
        .args(["-p", "ok", "--output-format", "json", "--max-turns", "1"])
        .env("CLAUDE_CODE_OAUTH_TOKEN", token)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();

    let Ok(child) = child else {
        return false;
    };

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut child = child;
        let _ = tx.send(child.wait());
    });

    match rx.recv_timeout(std::time::Duration::from_secs(15)) {
        Ok(Ok(status)) => status.success(),
        _ => false,
    }
}

/// Run Anthropic authentication setup.
///
/// Probes Claude CLI authentication first. If credentials are already valid the
/// step is auto-completed. Otherwise the user is guided through pasting an
/// OAuth token generated via `claude setup-token`.
///
/// Returns the OAuth token if one was provided and validated, so the caller
/// can persist it in config.toml.
pub(crate) fn run_anthropic_auth() -> anyhow::Result<Option<String>> {
    let spinner = cliclack::spinner();
    spinner.start("Checking Anthropic authentication...");
    let authenticated = is_claude_authenticated();

    if authenticated {
        spinner.stop("Anthropic authentication — already configured");
        return Ok(None);
    }

    spinner.stop("Anthropic authentication — not detected");

    let auth_method: &str = cliclack::select("Anthropic auth method")
        .item(
            "paste-token",
            "Paste OAuth token (Recommended)",
            "Run `claude setup-token` elsewhere, then paste the generated token here",
        )
        .item(
            "skip",
            "Skip for now",
            "Authenticate later with: claude login or claude setup-token",
        )
        .interact()?;

    if auth_method == "skip" {
        init_style::omega_warning(
            "You can authenticate later with: claude login or claude setup-token",
        )?;
        return Ok(None);
    }

    init_style::omega_note(
        "Anthropic OAuth token",
        "Run `claude setup-token` in another terminal.\n\
         Authorize in the browser, then paste the generated token below.",
    )?;

    const MAX_ATTEMPTS: u8 = 3;
    let mut last_token = String::new();

    for attempt in 1..=MAX_ATTEMPTS {
        let token: String = cliclack::input("Paste OAuth token")
            .placeholder("sk-ant-oat01-...")
            .validate(|input: &String| {
                if input.trim().is_empty() {
                    return Err("Token is required");
                }
                Ok(())
            })
            .interact()?;

        let token = token.trim().to_string();

        let spinner = cliclack::spinner();
        spinner.start("Validating OAuth token...");

        if validate_oauth_token(&token) {
            spinner.stop("Anthropic authentication — configured");
            return Ok(Some(token));
        }

        last_token = token;

        if attempt < MAX_ATTEMPTS {
            spinner.error(format!(
                "Token validation failed — attempt {attempt}/{MAX_ATTEMPTS}. Try again."
            ));
        } else {
            spinner.error("Token validation failed — claude could not authenticate");
            init_style::omega_warning(
                "The token will still be saved to config.toml.\n\
                 You can re-authenticate later with: claude login or claude setup-token",
            )?;
        }
    }

    Ok(Some(last_token))
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
