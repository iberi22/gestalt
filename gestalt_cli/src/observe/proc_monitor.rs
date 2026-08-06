use gestalt_router::event_bus::BusEvent;
use gestalt_router::run::AgentSpec;
use std::collections::HashMap;
use std::time::Instant;

pub const KNOWN_AGENTS: &[&str] = &[
    "opencode", "codex", "claude", "kimi", "agy", "hermes", "gestalt", "orca", "jules",
];

/// Pure function to match a command line (represented as slice of string slices) against known agents and AgentSpecs.
/// It returns Option<&str> representing either a matched AgentSpec id, or a static known agent binary name.
/// It NEVER matches generic node or python command lines.
pub fn match_agent<'a>(cmdline: &[&str], specs: &'a [AgentSpec]) -> Option<&'a str> {
    if cmdline.is_empty() {
        return None;
    }

    // First, let's find if any known agent keyword is matched as a substring in the command line.
    let mut matched_agent_name = None;
    for &agent in KNOWN_AGENTS {
        for &arg in cmdline {
            let lower_arg = arg.to_lowercase();
            // If the argument contains the agent keyword
            if lower_arg.contains(agent) {
                matched_agent_name = Some(agent);
                break;
            }
        }
        if matched_agent_name.is_some() {
            break;
        }
    }

    if let Some(agent_name) = matched_agent_name {
        // If we found a match, check if any of the specs has a matching agent id or command.
        for spec in specs {
            if spec.id.to_lowercase().contains(agent_name)
                || spec.command.to_lowercase().contains(agent_name)
            {
                return Some(&spec.id);
            }
        }
        // Since agent_name is a &'static str from KNOWN_AGENTS, it can be returned directly as &'a str.
        return Some(agent_name);
    }

    None
}

/// Linux process monitor that polls `/proc/*/cmdline` to track agent process lifecycle.
pub struct ProcMonitor {
    proc_path: std::path::PathBuf,
    tracked: HashMap<u32, (String, Instant, Vec<String>)>,
    specs: Vec<AgentSpec>,
}

impl ProcMonitor {
    pub fn new(specs: Vec<AgentSpec>) -> Self {
        Self {
            proc_path: std::path::PathBuf::from("/proc"),
            tracked: HashMap::new(),
            specs,
        }
    }

    /// Allows custom proc filesystem path for testing purposes.
    pub fn with_proc_path(mut self, path: std::path::PathBuf) -> Self {
        self.proc_path = path;
        self
    }

    /// Scan /proc and update tracked PID state, returning start/finish BusEvents.
    pub fn poll(&mut self) -> Vec<BusEvent> {
        let mut events = Vec::new();
        let mut active_pids = HashMap::new();

        if let Ok(entries) = std::fs::read_dir(&self.proc_path) {
            for entry in entries {
                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => continue,
                };
                let path = entry.path();
                let filename = match path.file_name().and_then(|f| f.to_str()) {
                    Some(name) => name,
                    None => continue,
                };

                // Check if directory name consists entirely of digits (PID)
                if !filename.chars().all(|c| c.is_ascii_digit()) {
                    continue;
                }

                let pid = match filename.parse::<u32>() {
                    Ok(p) => p,
                    Err(_) => continue,
                };

                // Read cmdline file
                let cmdline_path = path.join("cmdline");
                let cmdline_bytes = match std::fs::read(&cmdline_path) {
                    Ok(bytes) => bytes,
                    Err(_) => continue,
                };

                if cmdline_bytes.is_empty() {
                    continue;
                }

                // Split by NULL bytes
                let cmdline: Vec<String> = cmdline_bytes
                    .split(|&b| b == 0)
                    .filter(|slice| !slice.is_empty())
                    .map(|slice| String::from_utf8_lossy(slice).into_owned())
                    .collect();

                if cmdline.is_empty() {
                    continue;
                }

                let cmd_slices: Vec<&str> = cmdline.iter().map(|s| s.as_str()).collect();

                if let Some(agent_name) = match_agent(&cmd_slices, &self.specs) {
                    active_pids.insert(pid, (agent_name.to_string(), cmdline));
                }
            }
        }

        // 1. Detect starting processes
        for (pid, (agent_name, cmdline)) in &active_pids {
            if !self.tracked.contains_key(pid) {
                let start_time = Instant::now();
                self.tracked
                    .insert(*pid, (agent_name.clone(), start_time, cmdline.clone()));

                let summary = format!("Process started (PID {}): {}", pid, cmdline.join(" "));
                let event = BusEvent::new(agent_name, "run_started", summary)
                    .with_state("Running")
                    .with_metadata(serde_json::json!({
                        "pid": pid,
                        "cmdline": cmdline,
                    }));
                events.push(event);
            }
        }

        // 2. Detect finished processes
        let mut vanished = Vec::new();
        for (pid, (agent_name, start_time, cmdline)) in &self.tracked {
            if !active_pids.contains_key(pid) {
                vanished.push((*pid, agent_name.clone(), *start_time, cmdline.clone()));
            }
        }

        for (pid, agent_name, start_time, cmdline) in vanished {
            self.tracked.remove(&pid);
            let duration = start_time.elapsed();
            let summary = format!(
                "Process finished (PID {}). Duration: {}ms",
                pid,
                duration.as_millis()
            );
            let event = BusEvent::new(agent_name, "run_finished", summary)
                .with_state("Success")
                .with_metadata(serde_json::json!({
                    "pid": pid,
                    "cmdline": cmdline,
                    "duration_ms": duration.as_millis() as u64,
                    "exit_code": 0,
                }));
            events.push(event);
        }

        events
    }
}
