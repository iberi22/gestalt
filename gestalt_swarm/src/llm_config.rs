// ============================================================================
// LLM Provider Configuration
// ============================================================================

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LLMProviderType {
    #[serde(rename = "nvidia")]
    NVIDIA,
    #[serde(rename = "minimax")]
    MiniMax,
    #[serde(rename = "groq")]
    Groq,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfig {
    pub provider: LLMProviderType,
    pub model: String,
    pub api_key: String,
    pub extra: Option<serde_json::Value>,
}

impl LLMConfig {
    pub fn nvidia(model: &str, api_key: &str) -> Self {
        Self {
            provider: LLMProviderType::NVIDIA,
            model: model.to_string(),
            api_key: api_key.to_string(),
            extra: None,
        }
    }

    pub fn minimax(model: &str, api_key: &str, group_id: &str) -> Self {
        Self {
            provider: LLMProviderType::MiniMax,
            model: model.to_string(),
            api_key: api_key.to_string(),
            extra: Some(serde_json::json!({ "group_id": group_id })),
        }
    }

    pub fn groq(model: &str, api_key: &str) -> Self {
        Self {
            provider: LLMProviderType::Groq,
            model: model.to_string(),
            api_key: api_key.to_string(),
            extra: None,
        }
    }
}

// Load credentials from secure storage
pub fn load_credentials() -> Vec<LLMConfig> {
    let mut configs = Vec::new();

    // NVIDIA credentials
    if let Ok(key) = std::env::var("NVIDIA_API_KEY") {
        if !key.is_empty() {
            configs.push(LLMConfig::nvidia(
                "mistralai/devstral-2-123b-instruct-2512",
                &key
            ));
            configs.push(LLMConfig::nvidia(
                "meta/llama-3.3-70b-instruct",
                &key
            ));
        }
    }

    // MiniMax credentials
    if let (Ok(api_key), Ok(group_id)) = (
        std::env::var("MINIMAX_API_KEY"),
        std::env::var("MINIMAX_GROUP_ID"),
    ) {
        if !api_key.is_empty() {
            configs.push(LLMConfig::minimax("MiniMax-Text-01", &api_key, &group_id));
        }
    }

    // Groq credentials
    if let Ok(key) = std::env::var("GROQ_API_KEY") {
        if !key.is_empty() {
            configs.push(LLMConfig::groq("llama-3.3-70b-versatile", &key));
        }
    }

    configs
}

// Get provider display name
pub fn provider_name(provider: &LLMProviderType) -> &'static str {
    match provider {
        LLMProviderType::NVIDIA => "NVIDIA",
        LLMProviderType::MiniMax => "MiniMax",
        LLMProviderType::Groq => "Groq",
    }
}

// Show configured providers
pub fn show_configured_providers() {
    let creds = load_credentials();
    
    println!("\n🔑 Configured LLM Providers:");
    for config in &creds {
        let name = provider_name(&config.provider);
        let masked_key = format!("{}...{}", 
            &config.api_key[..4.min(config.api_key.len())],
            &config.api_key[config.api_key.len().saturating_sub(4)..]
        );
        println!("   • {} | {} | {}", name, config.model, masked_key);
    }
    
    if creds.is_empty() {
        println!("   ⚠️  No LLM providers configured!");
        println!("   Set environment variables:");
        println!("     NVIDIA_API_KEY");
        println!("     MINIMAX_API_KEY + MINIMAX_GROUP_ID");
        println!("     GROQ_API_KEY");
    }
    println!();
}
