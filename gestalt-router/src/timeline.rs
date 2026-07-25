use crate::run_state::AgentState;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

/// Helper function to retrieve the base path for runs.
///
/// It checks `$GESTALT_HOME` (and joins with `"runs"`) first,
/// then falls back to `~/.gestalt/runs/`.
pub fn get_base_dir() -> PathBuf {
    if let Some(gestalt_home) = std::env::var_os("GESTALT_HOME") {
        PathBuf::from(gestalt_home).join("runs")
    } else if let Some(home) = dirs::home_dir() {
        home.join(".gestalt").join("runs")
    } else {
        PathBuf::from(".gestalt").join("runs")
    }
}

/// An event tracked in the timeline of a run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "payload")]
pub enum Event {
    RunStarted {
        run_id: Uuid,
        task: String,
        agents: Vec<String>,
        sha_base: String,
    },
    AgentStateChanged {
        run_id: Uuid,
        agent_id: String,
        from: AgentState,
        to: AgentState,
    },
    CheckpointCommitted {
        commit_hash: String,
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
        file: String,
    },
    MergeComputed {
        target_branch: String,
        success: bool,
    },
    BranchPublished {
        branch: String,
    },
    SymlinkEscape {
        path: String,
    },
    ExcludedFile {
        path: String,
    },
    RunFinished {
        run_id: Uuid,
        summary: String,
    },
}

/// A wrapper to include a schema version in each logged event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VersionedEvent {
    pub v: usize,
    #[serde(flatten)]
    pub event: Event,
}

/// Trait defining the operations on an append-only timeline log.
pub trait EventLog {
    /// Appends an event to the log.
    fn append(&self, event: Event) -> Result<(), crate::run::RouterError>;

    /// Reads all successfully parsed events for a specific run.
    fn read_events(&self, run_id: Uuid) -> Result<Vec<Event>, crate::run::RouterError>;

    /// Lists all run IDs that have logs in the directory.
    fn list_runs(&self) -> Result<Vec<Uuid>, crate::run::RouterError>;
}

/// A JSON Lines (JSONL) implementation of the EventLog trait.
pub struct JsonlEventLog {
    _run_id: Uuid,
    base_dir: PathBuf,
    writer: Mutex<BufWriter<File>>,
}

impl JsonlEventLog {
    /// Creates a new `JsonlEventLog` with the default base directory.
    pub fn new(run_id: Uuid) -> Result<Self, crate::run::RouterError> {
        let base_dir = get_base_dir();
        Self::new_with_dir(run_id, base_dir)
    }

    /// Creates a new `JsonlEventLog` with a custom base directory (useful for testing).
    pub fn new_with_dir(run_id: Uuid, base_dir: PathBuf) -> Result<Self, crate::run::RouterError> {
        let run_dir = base_dir.join(run_id.to_string());
        fs::create_dir_all(&run_dir).map_err(|e| {
            crate::run::RouterError::TimelineError(format!("Failed to create directory: {}", e))
        })?;

        let file_path = run_dir.join("events.jsonl");
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .map_err(|e| {
                crate::run::RouterError::TimelineError(format!("Failed to open event log: {}", e))
            })?;

        let writer = Mutex::new(BufWriter::new(file));

        Ok(Self {
            _run_id: run_id,
            base_dir,
            writer,
        })
    }
}

impl EventLog for JsonlEventLog {
    fn log(&self, event: Event) -> Result<(), crate::run::RouterError> {
        self.append(event)
    }

    fn append(&self, event: Event) -> Result<(), crate::run::RouterError> {
        let versioned = VersionedEvent { v: 1, event };

        let mut serialized = serde_json::to_string(&versioned).map_err(|e| {
            crate::run::RouterError::TimelineError(format!("Serialization error: {}", e))
        })?;
        serialized.push('\n');

        let mut guard = self.writer.lock().map_err(|_| {
            crate::run::RouterError::TimelineError("Mutex poisoned".to_string())
        })?;

        guard.write_all(serialized.as_bytes()).map_err(|e| {
            crate::run::RouterError::TimelineError(format!("Write error: {}", e))
        })?;

        // Atomic/Durability pattern: flush BufWriter to file descriptor
        guard.flush().map_err(|e| {
            crate::run::RouterError::TimelineError(format!("Flush error: {}", e))
        })?;

        // Fsync pattern: persist to disk
        let file = guard.get_ref();
        if let Err(e) = file.sync_all() {
            // "fsync falla: Reportar error, continuar (mejor perder un evento que todo el run)"
            tracing::warn!("Failed to fsync event log: {}", e);
        }

        Ok(())
    }

    fn read_events(&self, run_id: Uuid) -> Result<Vec<Event>, crate::run::RouterError> {
        let run_dir = self.base_dir.join(run_id.to_string());
        let file_path = run_dir.join("events.jsonl");
        if !file_path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&file_path).map_err(|e| {
            crate::run::RouterError::TimelineError(format!(
                "Failed to open event log for reading: {}",
                e
            ))
        })?;

        let reader = BufReader::new(file);
        let mut lines = Vec::new();
        for line in reader.lines() {
            let line = line.map_err(|e| {
                crate::run::RouterError::TimelineError(format!("Read line error: {}", e))
            })?;
            lines.push(line);
        }

        let mut events = Vec::new();
        let len = lines.len();
        for (i, line) in lines.iter().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<VersionedEvent>(line) {
                Ok(versioned) => {
                    events.push(versioned.event);
                }
                Err(e) => {
                    if i == len - 1 {
                        // "Última línea truncada: Ignorar, loguear warning"
                        tracing::warn!("Truncated last line in event log ignored: {}", e);
                    } else {
                        return Err(crate::run::RouterError::TimelineError(format!(
                            "Parse error at line {}: {}",
                            i + 1,
                            e
                        )));
                    }
                }
            }
        }

        Ok(events)
    }

    fn list_runs(&self) -> Result<Vec<Uuid>, crate::run::RouterError> {
        if !self.base_dir.exists() {
            return Ok(Vec::new());
        }
        let mut run_ids = Vec::new();
        let entries = fs::read_dir(&self.base_dir).map_err(|e| {
            crate::run::RouterError::TimelineError(format!("Failed to read base directory: {}", e))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                crate::run::RouterError::TimelineError(format!("Dir entry error: {}", e))
            })?;
            let path = entry.path();
            if path.is_dir() {
                if let Some(name_str) = path.file_name().and_then(|s| s.to_str()) {
                    if let Ok(uuid) = Uuid::parse_str(name_str) {
                        if path.join("events.jsonl").exists() {
                            run_ids.push(uuid);
                        }
                    }
                }
            }
        }
        Ok(run_ids)
    }
}
