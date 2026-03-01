# Init Wizard Helpers (backend/src/init_wizard.rs)

## Overview

Interactive-only helpers extracted from `init.rs` to keep the init module under the 500-line limit. Contains browser detection, Anthropic authentication, WhatsApp QR pairing, and Google Workspace OAuth setup -- all of which require `cliclack` interactive prompts and are **not used** in non-interactive mode.

**Called by:** `backend/src/init.rs` (interactive wizard path only)

## Functions

### `detect_private_browsers() -> Vec<usize>`

Detects installed browsers on macOS that support incognito/private mode. Checks `/Applications/<browser>.app` for Chrome, Brave, Firefox, and Edge. Returns indices into the `PRIVATE_BROWSERS` constant.

Used to offer users an incognito browser option during Google OAuth (avoids cached session issues).

### `create_incognito_script(browser) -> Result<PathBuf>`

Creates a temporary shell script at `/tmp/omega_incognito_browser.sh` that opens a URL in the selected browser's incognito/private mode. Written with `0o700` permissions (TOCTOU-safe). The script is set as `$BROWSER` env var when launching the OAuth flow.

### `run_anthropic_auth() -> Result<()>`

Interactive Anthropic authentication. Offers two choices:
1. **Already authenticated** -- confirms and moves on
2. **Paste setup-token** -- prompts for a token, runs `claude setup-token <token>`, reports success or failure

### `run_whatsapp_setup() -> Result<bool>`

WhatsApp QR pairing flow:
1. If already paired (`~/.omega/whatsapp_session/whatsapp.db` exists), reports success and returns `true`
2. Asks user if they want to connect WhatsApp
3. Starts pairing bot, waits up to 30s for QR code
4. Renders QR in terminal via `cliclack::note()`
5. Waits up to 60s for scan completion
6. Returns `true` on success, `false` on failure/decline

### `run_google_setup() -> Result<Option<String>>`

Google Workspace OAuth setup via the `gog` CLI tool:
1. Checks if `gog` is installed (skips silently if not)
2. Asks user if they want to set up Google Workspace
3. Shows setup instructions (GCP console, API enabling, OAuth consent)
4. Prompts for `client_secret.json` file path
5. Runs `gog auth credentials <path>`
6. Prompts for Gmail address
7. Offers incognito browser option for OAuth flow
8. Runs `gog auth add <email> --services gmail,calendar,drive,contacts,docs,sheets`
9. Verifies with `gog auth list`
10. Returns `Some(email)` on success, `None` on failure/decline

## Data Structures

### `PrivateBrowser`
```rust
pub(crate) struct PrivateBrowser {
    pub label: &'static str,  // Display name (e.g., "Google Chrome")
    pub app: &'static str,    // macOS app name (e.g., "Google Chrome")
    pub flag: &'static str,   // Incognito flag (e.g., "--incognito")
}
```

### `PRIVATE_BROWSERS` (const)
Chrome, Brave, Firefox, Edge -- with their respective incognito/private flags.

## Visibility

All exports are `pub(crate)` -- only accessible within the binary crate, not exposed as public API.
