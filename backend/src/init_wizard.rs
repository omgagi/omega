//! Interactive-only helpers for the init wizard (browser detection, auth, WhatsApp, Google).

use crate::init_style;
use omega_channels::whatsapp;
use omega_core::shellexpand;
use std::path::Path;

/// Browser that supports private/incognito mode from the command line.
pub(crate) struct PrivateBrowser {
    pub label: &'static str,
    pub app: &'static str,
    pub flag: &'static str,
}

/// Known browsers with incognito/private mode support on macOS.
pub(crate) const PRIVATE_BROWSERS: &[PrivateBrowser] = &[
    PrivateBrowser {
        label: "Google Chrome",
        app: "Google Chrome",
        flag: "--incognito",
    },
    PrivateBrowser {
        label: "Brave",
        app: "Brave Browser",
        flag: "--incognito",
    },
    PrivateBrowser {
        label: "Firefox",
        app: "Firefox",
        flag: "--private-window",
    },
    PrivateBrowser {
        label: "Microsoft Edge",
        app: "Microsoft Edge",
        flag: "--inprivate",
    },
];

/// Detect installed browsers that support incognito/private mode (macOS).
///
/// Returns indices into `PRIVATE_BROWSERS` for browsers found in `/Applications`.
pub(crate) fn detect_private_browsers() -> Vec<usize> {
    PRIVATE_BROWSERS
        .iter()
        .enumerate()
        .filter(|(_, b)| Path::new(&format!("/Applications/{}.app", b.app)).exists())
        .map(|(i, _)| i)
        .collect()
}

/// Create a temporary shell script that opens a URL in incognito/private mode.
///
/// Returns the path to the script on success.
pub(crate) fn create_incognito_script(
    browser: &PrivateBrowser,
) -> anyhow::Result<std::path::PathBuf> {
    let script_path = std::env::temp_dir().join("omega_incognito_browser.sh");
    let script = format!(
        "#!/bin/sh\nopen -na '{}' --args {} \"$1\"\n",
        browser.app, browser.flag
    );
    // Create with restricted permissions first (0o700), then write content.
    // Prevents TOCTOU: no window where the file is world-readable.
    #[cfg(unix)]
    {
        use std::fs::OpenOptions;
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o700)
            .open(&script_path)?;
        f.write_all(script.as_bytes())?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(&script_path, script)?;
    }
    Ok(script_path)
}

/// Run Anthropic authentication setup.
///
/// Offers the user a choice between "already authenticated" and pasting a setup-token.
pub(crate) fn run_anthropic_auth() -> anyhow::Result<()> {
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
        init_style::omega_note(
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
                init_style::omega_warning("You can authenticate later with: claude setup-token")?;
            }
            Err(e) => {
                spinner.error(format!("Failed to run claude: {e}"));
                init_style::omega_warning("You can authenticate later with: claude setup-token")?;
            }
        }
    } else {
        init_style::omega_success("Anthropic authentication — already configured")?;
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

/// Run Google Workspace setup using the `gog` CLI tool.
///
/// Returns `Some(email)` if Google was successfully connected.
pub(crate) fn run_google_setup() -> anyhow::Result<Option<String>> {
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
    init_style::omega_note(
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
            init_style::omega_warning("Skipping Google Workspace setup.")?;
            return Ok(None);
        }
        Err(e) => {
            spinner.error(format!("Failed to run gog: {e}"));
            init_style::omega_warning("Skipping Google Workspace setup.")?;
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

    // Offer incognito browser for OAuth (avoids cached session issues).
    let browsers = detect_private_browsers();
    let incognito_script = if !browsers.is_empty() {
        let use_incognito: bool =
            cliclack::confirm("Open OAuth URL in incognito/private window? (recommended)")
                .initial_value(true)
                .interact()?;

        if use_incognito {
            let browser_idx = if browsers.len() == 1 {
                browsers[0]
            } else {
                let mut select = cliclack::select("Which browser?");
                for &idx in &browsers {
                    let b = &PRIVATE_BROWSERS[idx];
                    select = select.item(idx, b.label, "");
                }
                select.interact()?
            };

            match create_incognito_script(&PRIVATE_BROWSERS[browser_idx]) {
                Ok(path) => Some(path),
                Err(e) => {
                    init_style::omega_warning(&format!(
                        "Could not set up incognito browser: {e} — using default browser"
                    ))?;
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    // Show OAuth troubleshooting tips before starting the flow.
    init_style::omega_note(
        "OAuth Tips",
        "A browser will open for Google sign-in.\n\
         • Click 'Advanced' → 'Go to gog (unsafe)' → Allow\n\
         • If 'Access blocked: not verified', go to OAuth consent screen →\n\
           Audience → Publish app (or add yourself as a test user)",
    )?;

    // Run: gog auth add <email> --services gmail,calendar,drive,contacts,docs,sheets
    let spinner = cliclack::spinner();
    spinner.start("Waiting for OAuth approval in browser...");
    let mut cmd = std::process::Command::new("gog");
    cmd.args([
        "auth",
        "add",
        &email,
        "--services",
        "gmail,calendar,drive,contacts,docs,sheets",
    ]);
    if let Some(ref script_path) = incognito_script {
        cmd.env("BROWSER", script_path);
    }
    let auth_result = cmd.output();

    // Clean up temp script.
    if let Some(script_path) = incognito_script {
        let _ = std::fs::remove_file(script_path);
    }

    match auth_result {
        Ok(output) if output.status.success() => {
            spinner.stop("OAuth approved");
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            spinner.error(format!("gog auth add failed: {stderr}"));
            init_style::omega_warning(
                "If your browser showed an error, try manually in an incognito window:\n\
                 gog auth add <email> --services gmail,calendar,drive,contacts,docs,sheets",
            )?;
            return Ok(None);
        }
        Err(e) => {
            spinner.error(format!("Failed to run gog: {e}"));
            init_style::omega_warning("Google Workspace setup incomplete.")?;
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
            init_style::omega_success("Google Workspace connected!")?;
            return Ok(Some(email));
        }
    }

    // Verification couldn't confirm, but auth might still have worked.
    init_style::omega_warning(
        "Could not verify Google auth — check manually with 'gog auth list'.",
    )?;
    Ok(Some(email))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_private_browsers_constant_has_entries() {
        assert!(
            !PRIVATE_BROWSERS.is_empty(),
            "should have at least one browser defined"
        );
        for b in PRIVATE_BROWSERS {
            assert!(!b.label.is_empty(), "label must not be empty");
            assert!(!b.app.is_empty(), "app must not be empty");
            assert!(!b.flag.is_empty(), "flag must not be empty");
        }
    }

    #[test]
    fn test_detect_private_browsers_returns_valid_indices() {
        let indices = detect_private_browsers();
        for &idx in &indices {
            assert!(
                idx < PRIVATE_BROWSERS.len(),
                "index {idx} out of bounds for PRIVATE_BROWSERS"
            );
        }
    }

    #[test]
    fn test_create_incognito_script() {
        let browser = &PRIVATE_BROWSERS[0]; // Google Chrome
        let path = create_incognito_script(browser).expect("should create script");
        assert!(path.exists(), "script file should exist");

        let content = std::fs::read_to_string(&path).expect("should read script");
        assert!(content.starts_with("#!/bin/sh\n"), "should have shebang");
        assert!(
            content.contains(browser.app),
            "should contain browser app name"
        );
        assert!(
            content.contains(browser.flag),
            "should contain browser flag"
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::metadata(&path)
                .expect("should get metadata")
                .permissions();
            assert_eq!(perms.mode() & 0o700, 0o700, "script should be executable");
        }

        // Cleanup.
        let _ = std::fs::remove_file(path);
    }
}
