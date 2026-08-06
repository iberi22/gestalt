#![allow(dead_code)]

//! Artifact Ingestion for Observe Daemon
//!
//! Handles:
//! (a) Claude projects JSONL transcript tailing with inode + mtime + offset tracking.
//! (b) Hermes session SQLite DB polling (offset by rowid).
//! (c) Jules task tracking via GitHub API issues.

use gestalt_router::event_bus::BusEvent;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ClaudeLine {
    #[serde(rename = "type")]
    line_type: Option<String>,
    is_side_chain: Option<bool>,
    tool_name: Option<String>,
    timestamp: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FileState {
    pub inode: u64,
    pub mtime: i64,
    pub offset: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct IngestState {
    pub claude_files: HashMap<String, FileState>,
    pub last_hermes_rowid: i64,
    pub last_jules_updated_at: String,
}

/// Parse a single line from a Claude JSONL transcript.
/// If valid, constructs a `BusEvent` with `toolName` and `timestamp`.
pub fn parse_claude_line(line: &str) -> Option<BusEvent> {
    if line.trim().is_empty() {
        return None;
    }
    let parsed: ClaudeLine = serde_json::from_str(line).ok()?;
    let tool_name = parsed.tool_name?;
    let timestamp = parsed.timestamp?;
    let is_side_chain = parsed.is_side_chain.unwrap_or(false);

    let event_type = if is_side_chain {
        "tool_call".to_string()
    } else {
        "checkpoint".to_string()
    };

    let summary = format!("Claude executed tool: {}", tool_name);
    let event = BusEvent::new("claude", event_type, summary)
        .with_ts(timestamp)
        .with_metadata(serde_json::json!({
            "toolName": tool_name,
            "isSideChain": is_side_chain,
        }));

    Some(event)
}

/// Read new content from a file using an offset.
/// If the file is smaller than our current offset, resets offset to 0.
pub fn tail_file(path: &Path, offset: &mut u64) -> std::io::Result<Vec<String>> {
    let mut file = File::open(path)?;
    let metadata = file.metadata()?;
    let len = metadata.len();

    if len < *offset {
        *offset = 0;
    }

    file.seek(SeekFrom::Start(*offset))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    *offset = len;

    let content = String::from_utf8_lossy(&buffer);
    let lines = content
        .lines()
        .map(|s| s.to_string())
        .collect::<Vec<String>>();

    Ok(lines)
}

#[cfg(unix)]
fn get_file_metadata(path: &Path) -> std::io::Result<(u64, i64)> {
    use std::os::unix::fs::MetadataExt;
    let meta = fs::metadata(path)?;
    Ok((meta.ino(), meta.mtime()))
}

#[cfg(not(unix))]
fn get_file_metadata(path: &Path) -> std::io::Result<(u64, i64)> {
    let meta = fs::metadata(path)?;
    let mtime = meta
        .modified()?
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    Ok((0, mtime))
}

/// State storage file paths.
pub fn get_state_file_path() -> PathBuf {
    home::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".gestalt")
        .join("ingest_offsets.json")
}

pub fn load_ingest_state() -> IngestState {
    let path = get_state_file_path();
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(state) = serde_json::from_str(&content) {
                return state;
            }
        }
    }
    IngestState::default()
}

pub fn save_ingest_state(state: &IngestState) -> std::io::Result<()> {
    let path = get_state_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(state)?;
    fs::write(path, content)
}

/// Ingest all Claude Code JSONL transcripts in a directory, updating state.
pub fn ingest_claude_transcripts(projects_dir: &Path, state: &mut IngestState) -> Vec<BusEvent> {
    let mut events = Vec::new();
    if !projects_dir.is_dir() {
        return events;
    }

    let entries = match fs::read_dir(projects_dir) {
        Ok(e) => e,
        Err(_) => return events,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "jsonl") {
            let path_str = path.to_string_lossy().to_string();
            let meta_res = get_file_metadata(&path);
            if let Ok((inode, mtime)) = meta_res {
                let mut current_offset = 0;
                let mut needs_read = true;

                if let Some(existing) = state.claude_files.get(&path_str) {
                    if existing.inode == inode && existing.mtime == mtime {
                        // File hasn't changed
                        needs_read = false;
                    } else if existing.inode == inode {
                        // File mtime updated, same inode
                        current_offset = existing.offset;
                    }
                }

                if needs_read {
                    let mut offset = current_offset;
                    if let Ok(lines) = tail_file(&path, &mut offset) {
                        for line in lines {
                            if let Some(ev) = parse_claude_line(&line) {
                                events.push(ev);
                            }
                        }
                        // Update state
                        state.claude_files.insert(
                            path_str,
                            FileState {
                                inode,
                                mtime,
                                offset,
                            },
                        );
                    }
                }
            }
        }
    }

    events
}

/// Poll Hermes Session Database for new sessions, offset by rowid.
pub fn poll_hermes_sessions(db_path: &Path, last_rowid: &mut i64) -> Result<Vec<BusEvent>, String> {
    let mut events = Vec::new();
    if !db_path.exists() {
        return Ok(events);
    }

    let conn = rusqlite::Connection::open(db_path)
        .map_err(|e| format!("Failed to open Hermes DB: {}", e))?;

    let mut stmt = conn
        .prepare("SELECT rowid, id, name, state, created_at FROM sessions WHERE rowid > ? ORDER BY rowid ASC")
        .map_err(|e| format!("Query preparation failed: {}", e))?;

    let rows = stmt
        .query_map([*last_rowid], |row| {
            let rowid: i64 = row.get(0)?;
            let id: String = row.get(1)?;
            let name: String = row.get(2)?;
            let state: String = row.get(3)?;
            let created_at: String = row.get(4)?;
            Ok((rowid, id, name, state, created_at))
        })
        .map_err(|e| format!("Failed to execute sessions query: {}", e))?;

    for (rowid, id, name, state, created_at) in rows.flatten() {
        *last_rowid = (*last_rowid).max(rowid);

        let summary = format!("Hermes session started: {}", name);
        let ev = BusEvent::new("hermes", "run_started", summary)
            .with_run_id(id)
            .with_state(state)
            .with_ts(created_at);
        events.push(ev);
    }

    Ok(events)
}

/// Poll Jules tasks via GitHub API issues with the label 'jules'.
pub fn poll_jules_github_issues(
    owner: &str,
    repo: &str,
    token: Option<&str>,
    last_updated_at: &mut String,
) -> Result<Vec<BusEvent>, String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("gestalt-observe-daemon")
        .build()
        .map_err(|e| e.to_string())?;

    let api_base =
        std::env::var("GITHUB_API_URL").unwrap_or_else(|_| "https://api.github.com".to_string());
    let url = format!("{}/repos/{}/{}/issues", api_base, owner, repo);
    let mut req = client.get(&url).query(&[
        ("labels", "jules"),
        ("state", "all"),
        ("sort", "updated"),
        ("direction", "asc"),
    ]);

    if let Some(tok) = token {
        req = req.bearer_auth(tok);
    }

    let resp = req
        .send()
        .map_err(|e| format!("GitHub request failed: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("GitHub API returned error: {}", resp.status()));
    }

    #[derive(Deserialize, Debug)]
    struct GithubIssue {
        number: i64,
        title: String,
        state: String,
        updated_at: String,
        html_url: String,
        labels: Vec<serde_json::Value>,
    }

    let issues: Vec<GithubIssue> = resp
        .json()
        .map_err(|e| format!("Failed to parse GitHub JSON: {}", e))?;

    let mut events = Vec::new();
    let mut newest_ts = last_updated_at.clone();

    for issue in issues {
        if !last_updated_at.is_empty() && issue.updated_at <= *last_updated_at {
            continue;
        }

        if issue.updated_at > newest_ts {
            newest_ts = issue.updated_at.clone();
        }

        // Determine agent state based on issue state/labels
        let mut agent_state = "Running";
        for label in &issue.labels {
            if let Some(name) = label.get("name").and_then(|n| n.as_str()) {
                if name.eq_ignore_ascii_case("completed") {
                    agent_state = "Success";
                } else if name.eq_ignore_ascii_case("failed") {
                    agent_state = "Crashed";
                }
            }
        }
        if issue.state == "closed" && agent_state == "Running" {
            agent_state = "Success";
        }

        let summary = format!(
            "Jules task #{}: {} ({})",
            issue.number, issue.title, issue.state
        );
        let event_type = if issue.state == "closed" {
            "run_finished"
        } else {
            "run_started"
        };

        let ev = BusEvent::new("jules", event_type, summary)
            .with_run_id(format!("jules-issue-{}", issue.number))
            .with_state(agent_state)
            .with_ts(issue.updated_at.clone())
            .with_metadata(serde_json::json!({
                "issue_url": issue.html_url,
                "labels": issue.labels,
            }));

        events.push(ev);
    }

    *last_updated_at = newest_ts;
    Ok(events)
}

/// The main ingress/orchestration routine that executes once to poll all sources,
/// pushes any new events to the locally running Event Bus, and persists state.
pub fn ingest_artifacts() -> Result<(), String> {
    let mut state = load_ingest_state();
    let mut all_events = Vec::new();

    // (a) Claude Transcripts
    if let Some(home) = home::home_dir() {
        let claude_projects = home.join(".claude").join("projects");
        let claude_events = ingest_claude_transcripts(&claude_projects, &mut state);
        all_events.extend(claude_events);
    }

    // (b) Hermes Session DB
    if let Some(home) = home::home_dir() {
        let hermes_db = home.join(".hermes").join("sessions.db");
        if let Ok(hermes_events) = poll_hermes_sessions(&hermes_db, &mut state.last_hermes_rowid) {
            all_events.extend(hermes_events);
        }
    }

    // (c) Jules task tracking via GitHub API
    let owner = std::env::var("REPO_OWNER").unwrap_or_else(|_| "iberi22".to_string());
    let repo = std::env::var("REPO_NAME").unwrap_or_else(|_| "gestalt-rust".to_string());
    let token = std::env::var("GITHUB_TOKEN").ok();

    if let Ok(jules_events) = poll_jules_github_issues(
        &owner,
        &repo,
        token.as_deref(),
        &mut state.last_jules_updated_at,
    ) {
        all_events.extend(jules_events);
    }

    // Push new events to Event Bus (fail-open)
    if !all_events.is_empty() {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(3))
            .build()
            .map_err(|e| e.to_string())?;

        for ev in &all_events {
            let _ = client
                .post("http://127.0.0.1:8081/api/event")
                .json(ev)
                .send();
        }

        let _ = save_ingest_state(&state);
    }

    Ok(())
}
