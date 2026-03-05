//! OAuth helpers for the `/google` messenger wizard.
//!
//! Handles building the authorization URL, exchanging the auth code for tokens,
//! and fetching the user's email. No external CLI tools needed.

use omega_core::error::OmegaError;
use serde::Deserialize;

/// Google OAuth redirect URI (hosted callback page).
const REDIRECT_URI: &str = "https://omgagi.ai/oauth/callback/";

/// Google OAuth scopes covering all supported services.
const SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/gmail.modify",
    "https://www.googleapis.com/auth/calendar",
    "https://www.googleapis.com/auth/drive",
    "https://www.googleapis.com/auth/documents",
    "https://www.googleapis.com/auth/spreadsheets",
    "https://www.googleapis.com/auth/presentations",
    "https://www.googleapis.com/auth/forms.body",
    "https://mail.google.com/",
    "https://www.googleapis.com/auth/tasks",
    "https://www.googleapis.com/auth/contacts",
    "https://www.googleapis.com/auth/chat.messages",
];

/// Token response from Google's OAuth token endpoint.
#[derive(Debug, Deserialize)]
pub(super) struct TokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    #[allow(dead_code)]
    pub expires_in: Option<u64>,
}

/// Build the Google OAuth authorization URL.
///
/// Uses `access_type=offline` and `prompt=consent` to ensure a refresh_token
/// is always returned, even if the user has previously authorized.
pub(super) fn build_authorization_url(client_id: &str) -> String {
    let scope = SCOPES.join(" ");
    let encoded_scope = urlencoding::encode(&scope);
    let encoded_redirect = urlencoding::encode(REDIRECT_URI);
    let encoded_client_id = urlencoding::encode(client_id);

    format!(
        "https://accounts.google.com/o/oauth2/v2/auth\
         ?client_id={encoded_client_id}\
         &redirect_uri={encoded_redirect}\
         &response_type=code\
         &scope={encoded_scope}\
         &access_type=offline\
         &prompt=consent"
    )
}

/// Exchange an authorization code for access + refresh tokens.
pub(super) async fn exchange_code_for_tokens(
    client_id: &str,
    client_secret: &str,
    code: &str,
) -> Result<TokenResponse, OmegaError> {
    // URL-decode the code in case the user copied from the browser URL bar
    // (where '/' appears as '%2F'). Also trim whitespace.
    let decoded_code = urlencoding::decode(code.trim()).unwrap_or(std::borrow::Cow::Borrowed(code));
    let clean_code = decoded_code.trim();

    let client = reqwest::Client::new();

    let params = [
        ("code", clean_code),
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("redirect_uri", REDIRECT_URI),
        ("grant_type", "authorization_code"),
    ];

    let resp = client
        .post("https://oauth2.googleapis.com/token")
        .form(&params)
        .send()
        .await
        .map_err(|e| OmegaError::Channel(format!("token exchange request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(OmegaError::Channel(format!(
            "token exchange failed ({status}): {body}"
        )));
    }

    resp.json::<TokenResponse>()
        .await
        .map_err(|e| OmegaError::Channel(format!("token response parse failed: {e}")))
}

/// Fetch the authenticated user's email address using an access token.
pub(super) async fn fetch_user_email(access_token: &str) -> Result<String, OmegaError> {
    let client = reqwest::Client::new();

    let resp = client
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| OmegaError::Channel(format!("userinfo request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(OmegaError::Channel(format!(
            "userinfo failed ({status}): {body}"
        )));
    }

    #[derive(Deserialize)]
    struct UserInfo {
        email: Option<String>,
    }

    let info: UserInfo = resp
        .json()
        .await
        .map_err(|e| OmegaError::Channel(format!("userinfo parse failed: {e}")))?;

    info.email
        .ok_or_else(|| OmegaError::Channel("no email in userinfo response".into()))
}

/// Build a Google Cloud Console API Library URL for a given project and API.
pub(super) fn gcp_api_library_url(project: &str, api: &str) -> String {
    format!("https://console.cloud.google.com/apis/library/{api}?project={project}")
}

/// Build a Google Cloud Console URL for a given path and project.
pub(super) fn gcp_console_url(project: &str, path: &str) -> String {
    format!("https://console.cloud.google.com/{path}?project={project}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_authorization_url_contains_required_params() {
        let url = build_authorization_url("test-client-id");
        assert!(url.contains("client_id=test-client-id"));
        assert!(url.contains("redirect_uri="));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("access_type=offline"));
        assert!(url.contains("prompt=consent"));
        assert!(url.contains("scope="));
    }

    #[test]
    fn test_build_authorization_url_encodes_scopes() {
        let url = build_authorization_url("cid");
        // Scopes contain '/' and ':' which should be encoded.
        assert!(url.contains("gmail.modify"));
        assert!(url.contains("calendar"));
        assert!(url.contains("drive"));
    }

    #[test]
    fn test_build_authorization_url_uses_correct_redirect() {
        let url = build_authorization_url("cid");
        let encoded = urlencoding::encode(REDIRECT_URI);
        assert!(
            url.contains(&encoded.to_string()),
            "URL must contain encoded redirect URI"
        );
    }

    #[test]
    fn test_token_response_deserialization() {
        let json = r#"{"access_token":"at","refresh_token":"rt","expires_in":3600}"#;
        let resp: TokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.access_token, "at");
        assert_eq!(resp.refresh_token.as_deref(), Some("rt"));
        assert_eq!(resp.expires_in, Some(3600));
    }

    #[test]
    fn test_token_response_without_refresh_token() {
        let json = r#"{"access_token":"at","expires_in":3600}"#;
        let resp: TokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.access_token, "at");
        assert!(resp.refresh_token.is_none());
    }

    #[test]
    fn test_gcp_api_library_url() {
        let url = gcp_api_library_url("my-project", "gmail.googleapis.com");
        assert_eq!(
            url,
            "https://console.cloud.google.com/apis/library/gmail.googleapis.com?project=my-project"
        );
    }

    #[test]
    fn test_gcp_console_url() {
        let url = gcp_console_url("proj-1", "apis/credentials/consent");
        assert_eq!(
            url,
            "https://console.cloud.google.com/apis/credentials/consent?project=proj-1"
        );
    }

    #[test]
    fn test_scopes_include_all_services() {
        let scope_str = SCOPES.join(" ");
        assert!(scope_str.contains("gmail"));
        assert!(scope_str.contains("calendar"));
        assert!(scope_str.contains("drive"));
        assert!(scope_str.contains("documents"));
        assert!(scope_str.contains("spreadsheets"));
        assert!(scope_str.contains("presentations"));
        assert!(scope_str.contains("forms"));
        assert!(scope_str.contains("tasks"));
        assert!(scope_str.contains("contacts"));
        assert!(scope_str.contains("chat"));
    }
}
