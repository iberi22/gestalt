//! Qwen OAuth2 integration using Device Code flow.
//!
//! Implements the OAuth 2.0 Device Authorization Grant (RFC 8628)
//! for Qwen AI authentication.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;
use tracing::{debug, info};

/// Qwen OAuth Configuration
const QWEN_OAUTH_BASE_URL: &str = "https://chat.qwen.ai";
const QWEN_OAUTH_CLIENT_ID: &str = "f0304373b74a44d2b584a3fb70ca9e56";
const QWEN_OAUTH_SCOPE: &str = "openid profile email model.completion";

/// Qwen credentials structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QwenCredentials {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expiry_date: Option<i64>, // Unix timestamp in milliseconds
    pub token_type: Option<String>,
    pub resource_url: Option<String>,
}

/// Device authorization response from Qwen.
#[derive(Debug, Deserialize)]
pub struct DeviceAuthResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub expires_in: i64,
}

/// Token response from Qwen.
#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub token_type: Option<String>,
    pub expires_in: Option<i64>,
    pub resource_url: Option<String>,
    pub error: Option<String>,
}

/// Get the path to Qwen credentials file.
fn get_qwen_credentials_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".qwen").join("oauth_creds.json")
}

/// Load cached Qwen credentials.
pub async fn load_cached_credentials() -> Option<QwenCredentials> {
    let path = get_qwen_credentials_path();
    if let Ok(content) = fs::read_to_string(&path).await {
        match serde_json::from_str(&content) {
            Ok(creds) => {
                info!("Loaded Qwen credentials from {:?}", path);
                Some(creds)
            }
            Err(e) => {
                tracing::warn!("Failed to parse Qwen credentials: {}", e);
                None
            }
        }
    } else {
        None
    }
}

/// Save Qwen credentials.
pub async fn save_credentials(creds: &QwenCredentials) -> anyhow::Result<()> {
    let path = get_qwen_credentials_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }
    let content = serde_json::to_string_pretty(creds)?;
    fs::write(&path, content).await?;
    info!("Saved Qwen credentials to {:?}", path);
    Ok(())
}

/// Clear Qwen credentials.
pub async fn clear_credentials() -> anyhow::Result<()> {
    let path = get_qwen_credentials_path();
    if path.exists() {
        fs::remove_file(&path).await?;
        info!("Cleared Qwen credentials");
    }
    Ok(())
}

/// Generate PKCE code verifier and challenge.
fn generate_pkce_pair() -> (String, String) {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    use sha2::{Digest, Sha256};

    let verifier: String = (0..32)
        .map(|_| rand::random::<u8>())
        .map(|b| format!("{:02x}", b))
        .collect();

    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());

    (verifier, challenge)
}

/// Request device authorization from Qwen.
async fn request_device_authorization(code_challenge: &str) -> anyhow::Result<DeviceAuthResponse> {
    let client = reqwest::Client::new();
    let url = format!("{}/api/v1/oauth2/device/code", QWEN_OAUTH_BASE_URL);

    let params = [
        ("client_id", QWEN_OAUTH_CLIENT_ID),
        ("scope", QWEN_OAUTH_SCOPE),
        ("code_challenge", code_challenge),
        ("code_challenge_method", "S256"),
    ];

    let resp = client
        .post(&url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "application/json")
        .form(&params)
        .send()
        .await?;

    if !resp.status().is_success() {
        let text = resp.text().await?;
        anyhow::bail!("Qwen device authorization failed: {}", text);
    }

    let auth_resp: DeviceAuthResponse = resp.json().await?;
    Ok(auth_resp)
}

/// Poll for device token.
async fn poll_device_token(
    device_code: &str,
    code_verifier: &str,
) -> anyhow::Result<Option<QwenCredentials>> {
    let client = reqwest::Client::new();
    let url = format!("{}/api/v1/oauth2/token", QWEN_OAUTH_BASE_URL);

    let params = [
        ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ("client_id", QWEN_OAUTH_CLIENT_ID),
        ("device_code", device_code),
        ("code_verifier", code_verifier),
    ];

    let resp = client
        .post(&url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "application/json")
        .form(&params)
        .send()
        .await?;

    if resp.status().as_u16() == 400 {
        // Likely "authorization_pending" - user hasn't approved yet
        return Ok(None);
    }

    if !resp.status().is_success() {
        let text = resp.text().await?;
        anyhow::bail!("Qwen token request failed: {}", text);
    }

    let token_resp: TokenResponse = resp.json().await?;

    if let Some(access_token) = token_resp.access_token {
        let creds = QwenCredentials {
            access_token,
            refresh_token: token_resp.refresh_token,
            expiry_date: token_resp
                .expires_in
                .map(|e| chrono::Utc::now().timestamp_millis() + e * 1000),
            token_type: token_resp.token_type,
            resource_url: token_resp.resource_url,
        };
        Ok(Some(creds))
    } else {
        Ok(None)
    }
}

/// Run the Qwen OAuth device flow login.
pub async fn run_device_flow_login() -> anyhow::Result<QwenCredentials> {
    let (code_verifier, code_challenge) = generate_pkce_pair();

    println!("\n🔐 Starting Qwen device authorization...");

    let auth = request_device_authorization(&code_challenge).await?;

    println!("\n📱 Please visit this URL in your browser:");
    println!("\n{}\n", auth.verification_uri_complete);
    println!("Waiting for authorization...\n");

    // Try to open browser
    if let Err(e) = open::that(&auth.verification_uri_complete) {
        debug!("Failed to open browser: {}", e);
    }

    // Poll for token with exponential backoff
    let mut poll_interval = std::time::Duration::from_secs(2);
    let max_wait = std::time::Duration::from_secs(auth.expires_in as u64);
    let start = std::time::Instant::now();

    while start.elapsed() < max_wait {
        tokio::time::sleep(poll_interval).await;

        match poll_device_token(&auth.device_code, &code_verifier).await {
            Ok(Some(creds)) => {
                save_credentials(&creds).await?;
                println!("✅ Qwen authentication successful!");
                return Ok(creds);
            }
            Ok(None) => {
                debug!("Authorization pending...");
            }
            Err(e) => {
                if e.to_string().contains("slow_down") {
                    poll_interval =
                        std::cmp::min(poll_interval * 2, std::time::Duration::from_secs(10));
                    debug!("Slowing down poll interval to {:?}", poll_interval);
                } else {
                    return Err(e);
                }
            }
        }
    }

    anyhow::bail!("Qwen authentication timed out")
}

/// Refresh the Qwen access token.
pub async fn refresh_access_token(refresh_token: &str) -> anyhow::Result<QwenCredentials> {
    let client = reqwest::Client::new();
    let url = format!("{}/api/v1/oauth2/token", QWEN_OAUTH_BASE_URL);

    let params = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", QWEN_OAUTH_CLIENT_ID),
    ];

    let resp = client
        .post(&url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "application/json")
        .form(&params)
        .send()
        .await?;

    if !resp.status().is_success() {
        let text = resp.text().await?;
        anyhow::bail!("Qwen token refresh failed: {}", text);
    }

    let token_resp: TokenResponse = resp.json().await?;

    let access_token = token_resp
        .access_token
        .ok_or_else(|| anyhow::anyhow!("No access token in refresh response"))?;

    let creds = QwenCredentials {
        access_token,
        refresh_token: token_resp
            .refresh_token
            .or_else(|| Some(refresh_token.to_string())),
        expiry_date: token_resp
            .expires_in
            .map(|e| chrono::Utc::now().timestamp_millis() + e * 1000),
        token_type: token_resp.token_type,
        resource_url: token_resp.resource_url,
    };

    save_credentials(&creds).await?;
    info!("Refreshed Qwen access token");

    Ok(creds)
}

/// Get a valid Qwen access token, refreshing if necessary.
pub async fn get_valid_access_token() -> anyhow::Result<String> {
    let cached = load_cached_credentials().await.ok_or_else(|| {
        anyhow::anyhow!("No Qwen credentials found. Run the login command first.")
    })?;

    // Check if token is expired
    if let Some(expiry_date) = cached.expiry_date {
        let now = chrono::Utc::now().timestamp_millis();
        if now >= expiry_date - 60_000 {
            // Refresh 60 seconds before expiry
            if let Some(refresh_token) = &cached.refresh_token {
                info!("Qwen access token expired, refreshing...");
                let new_creds = refresh_access_token(refresh_token).await?;
                return Ok(new_creds.access_token);
            } else {
                anyhow::bail!("Qwen token expired and no refresh token available.");
            }
        }
    }

    Ok(cached.access_token)
}

/// Check if Qwen credentials exist.
pub async fn has_qwen_credentials() -> bool {
    load_cached_credentials().await.is_some()
}
