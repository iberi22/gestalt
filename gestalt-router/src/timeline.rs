use serde::{Serialize, Deserialize};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;
use crate::run_state::AgentState;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum Event {
    RunStarted {
        run_id: Uuid,
        sha_base: String,
        agents: Vec<String>,
        task: String,
    },
    AgentStateChanged {
        run_id: Uuid,
        agent: String,
        from: AgentState,
        to: AgentState,
    },
    OverlapDetected {
        run_id: Uuid,
        agent_a: String,
        agent_b: String,
        files: Vec<String>,
    },
    MergeConflict {
        run_id: Uuid,
        agent: String,
        path: String,
    },
    RunFinished {
        run_id: Uuid,
        summary: String,
    },
}

pub trait EventLog: Send + Sync {
    fn log(&self, event: Event) -> Result<(), String>;
}

pub struct JsonlEventLog {
    file: Mutex<File>,
    path: PathBuf,
}

impl JsonlEventLog {
    pub fn new(path: PathBuf) -> Result<Self, std::io::Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        Ok(Self {
            file: Mutex::new(file),
            path,
        })
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl EventLog for JsonlEventLog {
    fn log(&self, event: Event) -> Result<(), String> {
        let serialized = serde_json::to_string(&event)
            .map_err(|e| format!("Serialization error: {}", e))?;
        let mut file = self.file.lock().map_err(|_| "Failed to lock event log file".to_string())?;
        writeln!(file, "{}", serialized)
            .map_err(|e| format!("IO error writing event: {}", e))?;
        let _ = file.flush();
        Ok(())
    }
}

pub struct MockEventLog {
    events: Mutex<Vec<Event>>,
}

impl MockEventLog {
    pub fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }

    pub fn get_events(&self) -> Vec<Event> {
        self.events.lock().unwrap().clone()
    }
}

impl EventLog for MockEventLog {
    fn log(&self, event: Event) -> Result<(), String> {
        self.events.lock().unwrap().push(event);
        Ok(())
    }
}
