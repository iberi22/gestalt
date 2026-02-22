use oauth2::{
    basic::BasicClient, reqwest::async_http_client, AuthUrl, AuthorizationCode, ClientId,
    ClientSecret, CsrfToken, PkceCodeChallenge, Scope, TokenResponse, TokenUrl,
};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use url::Url;

use crate::adapters::auth::google_oauth::{save_credentials, GeminiCliToken};

pub struct GoogleAuthFlow {
    client: BasicClient,
}

impl Default for GoogleAuthFlow {
    fn default() -> Self {
        Self::new()
    }
}

impl GoogleAuthFlow {
    pub fn new() -> Self {
        let client_id = std::env::var("GOOGLE_OAUTH_CLIENT_ID").unwrap_or_else(|_| {
            "474415174526-j0j8j8j8j8j8j8j8j8j8j8j8j8j8j8j8.apps.googleusercontent.com".to_string()
        });
        let client_secret = std::env::var("GOOGLE_OAUTH_CLIENT_SECRET")
            .unwrap_or_else(|_| "YOUR_OAUTH_SECRET".to_string());

        let client = BasicClient::new(
            ClientId::new(client_id),
            Some(ClientSecret::new(client_secret)),
            AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string()).unwrap(),
            Some(TokenUrl::new("https://oauth2.googleapis.com/token".to_string()).unwrap()),
        );

        Self { client }
    }

    pub async fn login(&self) -> anyhow::Result<GeminiCliToken> {
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        let (auth_url, _csrf_token) = self
            .client
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new(
                "https://www.googleapis.com/auth/cloud-platform".to_string(),
            ))
            .add_scope(Scope::new(
                "https://www.googleapis.com/auth/userinfo.email".to_string(),
            ))
            .set_pkce_challenge(pkce_challenge)
            .url();

        println!("Open this URL in your browser:\n{}\n", auth_url);
        open::that(auth_url.as_str())?;

        // Start a local server to listen for the callback
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();

        // We need to update the redirect URL in the client or handled manually
        // For simplicity in this implementation, we assume the user might need to paste the code
        // OR we use a fixed port if configured in Google Console.

        println!("Waiting for callback on port {}...", port);

        if let Some(stream) = listener.incoming().next() {
            let mut stream = stream?;
            let mut reader = BufReader::new(&stream);
            let mut first_line = String::new();
            reader.read_line(&mut first_line)?;

            let auth_code = extract_code(&first_line)?;

            let message = "Authentication successful! You can close this window.";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                message.len(),
                message
            );
            stream.write_all(response.as_bytes())?;

            let token_result = self
                .client
                .exchange_code(AuthorizationCode::new(auth_code))
                .set_pkce_verifier(pkce_verifier)
                .request_async(async_http_client)
                .await?;

            let token = GeminiCliToken {
                access_token: token_result.access_token().secret().clone(),
                refresh_token: token_result.refresh_token().map(|t| t.secret().clone()),
                expiry_date: token_result
                    .expires_in()
                    .map(|d| chrono::Utc::now().timestamp_millis() + (d.as_secs() as i64 * 1000)),
                scope: None,
                token_type: Some("Bearer".to_string()),
                id_token: None,
            };

            save_credentials(&token).await?;
            return Ok(token);
        }

        anyhow::bail!("Login failed")
    }
}

fn extract_code(line: &str) -> anyhow::Result<String> {
    let url_parts: Vec<&str> = line.split_whitespace().collect();
    if url_parts.len() < 2 {
        anyhow::bail!("Invalid HTTP request");
    }

    let url = Url::parse(&format!("http://localhost{}", url_parts[1]))?;
    let code = url
        .query_pairs()
        .find(|(name, _)| name == "code")
        .map(|(_, value)| value.into_owned())
        .ok_or_else(|| anyhow::anyhow!("No code found in URL"))?;

    Ok(code)
}
