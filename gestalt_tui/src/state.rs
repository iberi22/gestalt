//! Application state shared across UI

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

/// Agent status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    Active,      // ● green
    Idle,        // ○ dark gray
    Processing,  // ◐ yellow
    Error,       // ✗ red
}

impl AgentStatus {
    pub fn symbol(&self) -> &'static str {
        match self {
            AgentStatus::Active => "●",
            AgentStatus::Idle => "○",
            AgentStatus::Processing => "◐",
            AgentStatus::Error => "✗",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            AgentStatus::Active => "active",
            AgentStatus::Idle => "idle",
            AgentStatus::Processing => "processing",
            AgentStatus::Error => "error",
        }
    }
}

/// Single agent entry
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub status: AgentStatus,
    pub last_seen: DateTime<Utc>,
    pub current_task: Option<String>,
}

impl Agent {
    pub fn new(id: &str, name: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            status: AgentStatus::Idle,
            last_seen: Utc::now(),
            current_task: None,
        }
    }

    pub fn with_status(mut self, status: AgentStatus) -> Self {
        self.status = status;
        self
    }
}

/// Log entry in output panel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub source: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    Success,
    Debug,
}

impl LogEntry {
    pub fn info(source: &str, msg: &str) -> Self {
        Self {
            timestamp: Utc::now(),
            level: LogLevel::Info,
            source: source.to_string(),
            message: msg.to_string(),
        }
    }

    pub fn success(source: &str, msg: &str) -> Self {
        Self {
            timestamp: Utc::now(),
            level: LogLevel::Success,
            source: source.to_string(),
            message: msg.to_string(),
        }
    }

    pub fn warn(source: &str, msg: &str) -> Self {
        Self {
            timestamp: Utc::now(),
            level: LogLevel::Warn,
            source: source.to_string(),
            message: msg.to_string(),
        }
    }

    pub fn error(source: &str, msg: &str) -> Self {
        Self {
            timestamp: Utc::now(),
            level: LogLevel::Error,
            source: source.to_string(),
            message: msg.to_string(),
        }
    }
}

/// Application global state
#[derive(Debug, Clone, Default)]
pub struct AppState {
    pub agents: Vec<Agent>,
    pub logs: Vec<LogEntry>,
    pub input_buffer: String,
    pub current_goal: Option<String>,
    pub connected: bool,
    pub scroll_offset: usize,
    pub agent_scroll: usize,
}

impl AppState {
    pub fn new() -> Self {
        let agents = vec![
            Agent::new("agent-1", "Coordinator").with_status(AgentStatus::Idle),
            Agent::new("agent-2", "Code Analyzer").with_status(AgentStatus::Idle),
            Agent::new("agent-3", "Task Planner").with_status(AgentStatus::Idle),
            Agent::new("agent-4", "Memory").with_status(AgentStatus::Idle),
        ];

        let logs = vec![
            LogEntry::info("system", "🐝 Gestalt Swarm TUI initialized"),
            LogEntry::info("system", "Swarm status: ready"),
            LogEntry::info("system", "4 agents online"),
        ];

        Self {
            agents,
            logs,
            ..Default::default()
        }
    }

    pub fn add_log(&mut self, entry: LogEntry) {
        self.logs.push(entry);
        // Keep last 1000 entries
        if self.logs.len() > 1000 {
            self.logs.remove(0);
        }
    }

    pub fn submit_goal(&mut self, goal: String) {
        self.current_goal = Some(goal.clone());
        self.add_log(LogEntry::info("user", &format!("> {}", goal)));
        self.add_log(LogEntry::info("swarm", "Processing goal..."));

        // Mark agents as processing
        for agent in &mut self.agents {
            agent.status = AgentStatus::Processing;
            agent.current_task = Some(goal.clone());
        }
    }

    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    pub fn active_count(&self) -> usize {
        self.agents
            .iter()
            .filter(|a| a.status == AgentStatus::Active || a.status == AgentStatus::Processing)
            .count()
    }
}

/// Thread-safe shared state
pub type SharedState = Arc<RwLock<AppState>>;

pub fn new_shared_state() -> SharedState {
    Arc::new(RwLock::new(AppState::new()))
}
