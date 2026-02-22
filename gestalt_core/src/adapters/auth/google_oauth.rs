//! Google OAuth2 integration for Gemini authentication.
//!
//! Reuses tokens from `gemini-cli` (stored in ~/.gemini/oauth_creds.json)
//! instead of implementing our own OAuth flow.

use oauth2::{
    basic::BasicClient, reqwest::async_http_client, ClientId, ClientSecret, RefreshToken,
    TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;
use tracing::info;

/// Get OAuth Client ID from environment variable.
/// Set GOOGLE_OAUTH_CLIENT_ID in your environment or .env file.
fn get_oauth_client_id() -> String {
    std::env::var("GOOGLE_OAUTH_CLIENT_ID").unwrap_or_else(|_| {
        tracing::warn!("GOOGLE_OAUTH_CLIENT_ID not set, using default");
        "YOUR_CLIENT_ID_HERE".to_string()
    })
}

/// Get OAuth Client Secret from environment variable.
/// Set GOOGLE_OAUTH_CLIENT_SECRET in your environment or .env file.
fn get_oauth_client_secret() -> String {
    std::env::var("GOOGLE_OAUTH_CLIENT_SECRET").unwrap_or_else(|_| {
        tracing::warn!("GOOGLE_OAUTH_CLIENT_SECRET not set, using default");
        "YOUR_CLIENT_SECRET_HERE".to_string()
    })
}

/// Token structure matching gemini-cli's oauth_creds.json format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiCliToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expiry_date: Option<i64>, // Unix timestamp in milliseconds
    pub scope: Option<String>,
    pub token_type: Option<String>,
    pub id_token: Option<String>,
}

/// Get the path to gemini-cli's cached credentials file.
fn get_gemini_cli_credentials_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".gemini").join("oauth_creds.json")
}

/// Load cached credentials from gemini-cli.
pub async fn load_cached_credentials() -> Option<GeminiCliToken> {
    let path = get_gemini_cli_credentials_path();
    if let Ok(content) = fs::read_to_string(&path).await {
        match serde_json::from_str(&content) {
            Ok(token) => {
                info!("Loaded Gemini CLI credentials from {:?}", path);
                Some(token)
            }
            Err(e) => {
                tracing::warn!("Failed to parse gemini-cli credentials: {}", e);
                None
            }
        }
    } else {
        None
    }
}

/// Save credentials back to gemini-cli's file.
pub async fn save_credentials(token: &GeminiCliToken) -> anyhow::Result<()> {
    let path = get_gemini_cli_credentials_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }
    let content = serde_json::to_string_pretty(token)?;
    fs::write(&path, content).await?;
    info!("Saved OAuth credentials to {:?}", path);
    Ok(())
}

/// Clear cached credentials (not typically needed since we share with gemini-cli).
pub async fn clear_credentials() -> anyhow::Result<()> {
    let path = get_gemini_cli_credentials_path();
    if path.exists() {
        fs::remove_file(&path).await?;
        info!("Cleared cached OAuth credentials");
    }
    Ok(())
}

/// Build the OAuth2 client for token refresh.
fn build_oauth_client() -> BasicClient {
    BasicClient::new(
        ClientId::new(get_oauth_client_id()),
        Some(ClientSecret::new(get_oauth_client_secret())),
        oauth2::AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string()).unwrap(),
        Some(TokenUrl::new("https://oauth2.googleapis.com/token".to_string()).unwrap()),
    )
}

/// Refresh the access token using the refresh token.
pub async fn refresh_access_token(refresh_token: &str) -> anyhow::Result<GeminiCliToken> {
    let client = build_oauth_client();

    let token_result = client
        .exchange_refresh_token(&RefreshToken::new(refresh_token.to_string()))
        .request_async(async_http_client)
        .await?;

    let new_token = GeminiCliToken {
        access_token: token_result.access_token().secret().clone(),
        refresh_token: token_result
            .refresh_token()
            .map(|t| t.secret().clone())
            .or_else(|| Some(refresh_token.to_string())), // Keep old refresh token if not returned
        expiry_date: token_result
            .expires_in()
            .map(|d| chrono::Utc::now().timestamp_millis() + (d.as_secs() as i64 * 1000)),
        scope: None,
        token_type: Some("Bearer".to_string()),
        id_token: None,
    };

    save_credentials(&new_token).await?;
    info!("Refreshed access token successfully");

    Ok(new_token)
}

/// Get a valid access token, refreshing if necessary.
pub async fn get_valid_access_token() -> anyhow::Result<String> {
    let cached = load_cached_credentials().await.ok_or_else(|| {
        anyhow::anyhow!("No Gemini CLI credentials found. Run `gemini` CLI to login first.")
    })?;

    // Check if token is expired (expiry_date is in milliseconds)
    if let Some(expiry_date) = cached.expiry_date {
        let now = chrono::Utc::now().timestamp_millis();
        if now >= expiry_date - 60_000 {
            // Refresh 60 seconds before expiry
            if let Some(refresh_token) = &cached.refresh_token {
                info!("Access token expired, refreshing...");
                let new_token = refresh_access_token(refresh_token).await?;
                return Ok(new_token.access_token);
            } else {
                anyhow::bail!(
                    "Token expired and no refresh token available. Run `gemini` CLI again."
                );
            }
        }
    }

    Ok(cached.access_token)
}

/// Check if gemini-cli credentials exist.
pub async fn has_gemini_cli_credentials() -> bool {
    load_cached_credentials().await.is_some()
}
