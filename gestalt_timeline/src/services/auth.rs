//! Google OAuth2 Authentication Service
//!
//! Handles the OAuth2 flow for Google Gemini API access.
//! Implements the "Installed Application" flow with a local loopback server.

use anyhow::{Context, Result};
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge, RedirectUrl,
    RefreshToken, Scope, TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tracing::{info, warn};

// Constants from gemini-cli reference implementation
const GOOGLE_CLIENT_ID: &str = "681255809395-oo8ft2oprdrnp9e3aqf6av3hmdib135j.apps.googleusercontent.com";
const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

// Scopes required for Gemini
const SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/cloud-platform",
    "https://www.googleapis.com/auth/userinfo.email",
    "https://www.googleapis.com/auth/userinfo.profile",
];

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuthCredentials {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Clone)]
pub struct AuthService {
    client: BasicClient,
}

impl AuthService {
    pub fn new() -> Result<Self> {
        let client_secret = std::env::var("GOOGLE_CLIENT_SECRET")
            .unwrap_or_else(|_| "PLACEHOLDER_SECRET".to_string()); // Fallback or fail? Prefer fallback to allow build, but runtime will fail if real secret needed.

        let client = BasicClient::new(
            ClientId::new(GOOGLE_CLIENT_ID.to_string()),
            Some(ClientSecret::new(client_secret)),
            AuthUrl::new(AUTH_URL.to_string())?,
            Some(TokenUrl::new(TOKEN_URL.to_string())?),
        )
        .set_redirect_uri(RedirectUrl::new("http://localhost:3000".to_string())?);

        Ok(Self { client })
    }

    /// Interactice login flow: opens browser, listents on port 3000, exchanges code.
    pub async fn login(&self) -> Result<AuthCredentials> {
        // Create a PKCE code verifier and challenge
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        // Generate the authorization URL
        let mut auth_req = self.client.authorize_url(CsrfToken::new_random);

        for scope in SCOPES {
            auth_req = auth_req.add_scope(Scope::new(scope.to_string()));
        }

        let (auth_url, _csrf_token) = auth_req
            .set_pkce_challenge(pkce_challenge)
            .url();

        println!("🚀 Opening browser to authenticate with Google...");
        println!("If it doesn't open, visit: {}", auth_url);

        // Open browser
        if let Err(e) = open::that(auth_url.to_string()) {
            warn!("Failed to open browser: {}", e);
        }

        // Start a local server to listen for the callback
        let code = self.listen_for_callback().await?;

        // Exchange the code for a token
        let token_result = self
            .client
            .exchange_code(AuthorizationCode::new(code))
            .set_pkce_verifier(pkce_verifier)
            .request_async(async_http_client)
            .await
            .context("Failed to exchange authorization code for token")?;

        let credentials = AuthCredentials {
            access_token: token_result.access_token().secret().to_string(),
            refresh_token: token_result.refresh_token().map(|t| t.secret().to_string()),
            expires_at: token_result.expires_in().map(|dur| {
                chrono::Utc::now() + chrono::Duration::from_std(dur).unwrap()
            }),
        };

        self.save_credentials(&credentials)?;
        info!("✅ Authentication successful!");

        Ok(credentials)
    }

    /// Starts a temporary TCP listener to capture the OAuth callback code
    async fn listen_for_callback(&self) -> Result<String> {
        let listener = TcpListener::bind("127.0.0.1:3000").await?;
        info!("Listening on http://localhost:3000 for callback...");

        let (tx, mut rx) = mpsc::channel(1);

        // Accept a single connection
        // In a real app we'd want a proper server, but for one-off CLI auth:
        tokio::select! {
            result = async {
                if let Ok((mut stream, _)) = listener.accept().await {
                    let mut buffer = [0; 2048];
                    if let Ok(n) = tokio::io::AsyncReadExt::read(&mut stream, &mut buffer).await {
                        let request = String::from_utf8_lossy(&buffer[..n]);

                        // Basic parsing to find ?code=...
                        // Request looks like: GET /?code=4/0.... HTTP/1.1
                        if let Some(start) = request.find("code=") {
                            let remainder = &request[start + 5..];
                            let end = remainder.find('&').or_else(|| remainder.find(' ')).unwrap_or(remainder.len());
                            let code = &remainder[..end];

                            // Send success response
                            let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<h1>Authentication Successful!</h1><p>You can close this window and return to the CLI.</p>";
                            let _ = tokio::io::AsyncWriteExt::write_all(&mut stream, response.as_bytes()).await;

                            return Ok(code.to_string());
                        }
                    }
                }
                Err(anyhow::anyhow!("Failed to parse callback request"))
            } => {
                let code = result?;
                let _ = tx.send(code).await;
            }
            _ = tokio::time::sleep(std::time::Duration::from_secs(120)) => {
                return Err(anyhow::anyhow!("Authentication timed out"));
            }
        }

        rx.recv().await.context("Failed to receive auth code")
    }

    /// Get valid access token, refreshing if necessary
    pub async fn get_valid_token(&self) -> Result<String> {
        let creds = self.load_credentials().context("No credentials found. Run 'gestalt login' first.")?;

        // Check if expired (with 1 minute buffer)
        let is_expired = creds.expires_at
            .map(|exp| exp < chrono::Utc::now() + chrono::Duration::minutes(1))
            .unwrap_or(true); // Assume expired if no expiration set (safer)

        if !is_expired {
            return Ok(creds.access_token);
        }

        if let Some(ref refresh_token) = creds.refresh_token {
            info!("🔄 Refreshing expired access token...");
            let token_result = self.client
                .exchange_refresh_token(&RefreshToken::new(refresh_token.clone()))
                .request_async(async_http_client)
                .await
                .context("Failed to refresh token")?;

            let new_creds = AuthCredentials {
                access_token: token_result.access_token().secret().to_string(),
                // Keep old refresh token if new one not provided (standard OAuth2 behavior)
                refresh_token: token_result.refresh_token().map(|t| t.secret().to_string()).or(creds.refresh_token),
                expires_at: token_result.expires_in().map(|dur| {
                    chrono::Utc::now() + chrono::Duration::from_std(dur).unwrap()
                }),
            };

            self.save_credentials(&new_creds)?;
            Ok(new_creds.access_token)
        } else {
            Err(anyhow::anyhow!("Token expired and no refresh token available. Please login again."))
        }
    }

    fn get_credentials_path() -> Result<PathBuf> {
        let mut path = dirs::home_dir().context("Could not find home directory")?;
        path.push(".gestalt");
        std::fs::create_dir_all(&path)?;
        path.push("gemini_credentials.json");
        Ok(path)
    }

    fn save_credentials(&self, creds: &AuthCredentials) -> Result<()> {
        let path = Self::get_credentials_path()?;
        let json = serde_json::to_string_pretty(creds)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    fn load_credentials(&self) -> Result<AuthCredentials> {
        let path = Self::get_credentials_path()?;
        let content = std::fs::read_to_string(path)?;
        let creds: AuthCredentials = serde_json::from_str(&content)?;
        Ok(creds)
    }
}
