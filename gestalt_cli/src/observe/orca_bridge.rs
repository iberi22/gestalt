//! Orca Agent-Hooks Bridge
//!
//! Exposes functions to read and poll Orca's local agent status endpoint,
//! and fallback to a local JSON file (`last-status.json`) when unreachable.

#![allow(dead_code)]

use gestalt_router::event_bus::BusEvent;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Status entry from Orca's agent hooks.
/// Supports multiple field aliases to maximize parsing robustness.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrcaAgentStatus {
    pub agent: String,

    #[serde(alias = "status")]
    pub state: Option<String>,

    #[serde(alias = "message")]
    pub summary: Option<String>,

    pub run_id: Option<String>,
    pub project: Option<String>,

    #[serde(alias = "timestamp")]
    pub ts: Option<String>,
}

/// Extracted endpoint configuration from Orca's hook configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrcaEndpointConfig {
    pub port: u16,
    pub token: String,
}

/// Pure parser to read the endpoint environment file format (KEY=VALUE lines).
pub fn read_orca_endpoint(path: &Path) -> Result<OrcaEndpointConfig, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read Orca endpoint file: {}", e))?;

    let mut port = None;
    let mut token = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some((key, val)) = trimmed.split_once('=') {
            let k = key.trim();
            let mut v = val.trim().to_string();

            // Strip surrounding double or single quotes
            if ((v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\''))) && v.len() >= 2 {
                v = v[1..v.len() - 1].to_string();
            }

            match k {
                "ORCA_AGENT_HOOK_PORT" => {
                    if let Ok(p) = v.parse::<u16>() {
                        port = Some(p);
                    } else {
                        return Err(format!("Invalid port value: {}", v));
                    }
                }
                "ORCA_AGENT_HOOK_TOKEN" | "TOKEN" => {
                    token = Some(v);
                }
                _ => {}
            }
        }
    }

    let port = port.ok_or_else(|| "ORCA_AGENT_HOOK_PORT not found in endpoint config".to_string())?;
    let token = token.unwrap_or_default();

    Ok(OrcaEndpointConfig { port, token })
}

/// Convert an OrcaAgentStatus entry to a canonical BusEvent.
pub fn to_bus_event(status: &OrcaAgentStatus) -> BusEvent {
    let agent = status.agent.clone();
    let raw_state = status.state.clone();

    // Normalize state to standard capitalization
    let normalized_state = match raw_state.as_deref().map(|s| s.to_lowercase()).as_deref() {
        Some("running") => Some("Running".to_string()),
        Some("success") | Some("succeeded") => Some("Success".to_string()),
        Some("timeout") | Some("timed_out") => Some("Timeout".to_string()),
        Some("crashed") | Some("failed") => Some("Crashed".to_string()),
        Some("pending") => Some("Pending".to_string()),
        Some("nochanges") | Some("no_changes") => Some("NoChanges".to_string()),
        Some("quarantined") => Some("Quarantined".to_string()),
        _ => raw_state.clone(),
    };

    let event_type = match normalized_state.as_deref() {
        Some("Running") => "run_started".to_string(),
        Some("Success") | Some("Crashed") | Some("Timeout") | Some("NoChanges") | Some("Quarantined") => "run_finished".to_string(),
        _ => "agent_state".to_string(),
    };

    let summary = status
        .summary
        .clone()
        .unwrap_or_else(|| format!("Orca agent status update: {}", agent));

    let mut event = BusEvent::new(agent, event_type, summary);

    if let Some(ref st) = normalized_state {
        event = event.with_state(st.clone());
    }
    if let Some(ref rid) = status.run_id {
        event = event.with_run_id(rid.clone());
    }
    if let Some(ref proj) = status.project {
        event = event.with_project(proj.clone());
    }
    if let Some(ref timestamp) = status.ts {
        event = event.with_ts(timestamp.clone());
    }

    // Embed raw status as metadata
    if let Ok(metadata_val) = serde_json::to_value(status) {
        event = event.with_metadata(metadata_val);
    }

    event
}

/// Fallback path reader: parses last-status.json file if the endpoint does not respond or fails.
pub fn read_last_status(path: &Path) -> Result<Vec<BusEvent>, String> {
    if !path.exists() {
        tracing::warn!("last-status.json does not exist at {}", path.display());
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read last-status.json: {}", e))?;

    let entries: Vec<OrcaAgentStatus> = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse last-status.json: {}", e))?;

    let events = entries.iter().map(to_bus_event).collect();
    Ok(events)
}

/// Polls Orca's HTTP endpoint, with graceful fallback to `last-status.json` on error or timeout.
pub async fn poll_orca() -> Result<Vec<BusEvent>, String> {
    let home = home::home_dir().ok_or_else(|| "Could not determine home directory".to_string())?;
    let agent_hooks_dir = home.join(".config/orca/agent-hooks");
    let env_path = agent_hooks_dir.join("endpoint.env");
    let fallback_path = agent_hooks_dir.join("last-status.json");

    // Try reading configuration
    let config = match read_orca_endpoint(&env_path) {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::warn!("Could not read Orca endpoint config: {}. Falling back.", e);
            return read_last_status(&fallback_path);
        }
    };

    // Construct URL and attempt polling
    let url = format!("http://127.0.0.1:{}/status", config.port);
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to build reqwest client: {}. Falling back.", e);
            return read_last_status(&fallback_path);
        }
    };

    let mut request = client.get(&url);
    if !config.token.is_empty() {
        request = request.bearer_auth(&config.token);
    }

    match request.send().await {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<Vec<OrcaAgentStatus>>().await {
                    Ok(entries) => {
                        let events = entries.iter().map(to_bus_event).collect();
                        Ok(events)
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse Orca response JSON: {}. Falling back.", e);
                        read_last_status(&fallback_path)
                    }
                }
            } else {
                tracing::warn!("Orca endpoint returned failure status {}. Falling back.", response.status());
                read_last_status(&fallback_path)
            }
        }
        Err(e) => {
            tracing::warn!("Orca endpoint unreachable: {}. Falling back.", e);
            read_last_status(&fallback_path)
        }
    }
}

/// Bridge Orca status updates into the standard Gestalt state database and Xavier sink.
pub async fn bridge_orca() -> Result<(), String> {
    let db_path = home::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".gestalt")
        .join("state.db");

    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let db = std::sync::Arc::new(
        gestalt_state::StateDb::open(&db_path)
            .map_err(|e| format!("Failed to open StateDb: {}", e))?,
    );

    let sink = std::env::var("XAVIER_TOKEN")
        .ok()
        .filter(|t| !t.is_empty())
        .map(|_| gestalt_router::xavier_sink::XavierEventSink::from_env());

    let events = poll_orca().await.unwrap_or_default();

    for ev in events {
        let _ = gestalt_router::event_bus::handle_event(&db, &ev, sink.as_ref()).await;
    }

    Ok(())
}
