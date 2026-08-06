//! Declarative TOML spec parsing and topological execution for agent chains
//!
//! Implements `gestalt chain run --spec pipeline.toml` sequential pipelines.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, info, warn};

use gestalt_router::event_bus::{BusEvent, handle_event};
use gestalt_state::StateDb;
use crate::agent_wrapper::{AgentWrapper, InMemoryVfs, BlockEdit};

/// Default event to trigger subsequent steps
fn default_on_event() -> String {
    "run_finished".to_string()
}

/// A single step in the pipeline chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainStep {
    /// Unique name of this step
    pub name: String,
    /// Agent to execute (e.g. "opencode", "codex")
    pub agent: String,
    /// Task description to pass to the agent
    pub task: String,
    /// Event type that triggers this step (defaults to "run_finished")
    #[serde(default = "default_on_event")]
    pub on_event: String,
    /// Dependencies on prior steps (by name)
    #[serde(default)]
    pub requires: Vec<String>,
}

/// Declarative TOML pipeline specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainSpec {
    /// Ordered or unordered steps list to be topologically sorted
    pub steps: Vec<ChainStep>,
}

/// Sort steps topologically based on their `requires` dependencies.
/// Detects missing dependencies and circular dependency cycles.
pub fn topological_sort(steps: &[ChainStep]) -> Result<Vec<ChainStep>, String> {
    let mut steps_map: HashMap<String, &ChainStep> = HashMap::new();
    for step in steps {
        if steps_map.insert(step.name.clone(), step).is_some() {
            return Err(format!("Duplicate step name '{}' in specification", step.name));
        }
    }

    // Verify all dependency references exist
    for step in steps {
        for req in &step.requires {
            if !steps_map.contains_key(req) {
                return Err(format!(
                    "Step '{}' requires undefined step '{}'",
                    step.name, req
                ));
            }
        }
    }

    // Kahn's algorithm setup
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();

    for step in steps {
        in_degree.insert(step.name.clone(), step.requires.len());
        for req in &step.requires {
            adjacency
                .entry(req.clone())
                .or_default()
                .push(step.name.clone());
        }
    }

    let mut queue: VecDeque<String> = steps
        .iter()
        .filter(|s| s.requires.is_empty())
        .map(|s| s.name.clone())
        .collect();

    let mut sorted_names = Vec::new();

    while let Some(node) = queue.pop_front() {
        sorted_names.push(node.clone());
        if let Some(neighbors) = adjacency.get(&node) {
            for neighbor in neighbors {
                if let Some(deg) = in_degree.get_mut(neighbor) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(neighbor.clone());
                    }
                }
            }
        }
    }

    if sorted_names.len() != steps.len() {
        return Err("Circular dependency or cycle detected in step requirements".to_string());
    }

    // Map names back to cloned ChainSteps
    let mut sorted_steps = Vec::new();
    for name in sorted_names {
        if let Some(step) = steps_map.get(&name) {
            sorted_steps.push((*step).clone());
        }
    }

    Ok(sorted_steps)
}

/// Execute a declarative pipeline chain of agents sequentially in topological order.
pub async fn run_chain(
    spec_path: &str,
    project: Option<String>,
    continue_on_error: bool,
) -> Result<(), String> {
    info!("Loading chain specification from {}", spec_path);
    let content = fs::read_to_string(spec_path)
        .map_err(|e| format!("Failed to read spec file '{}': {}", spec_path, e))?;

    let spec: ChainSpec = toml::from_str(&content)
        .map_err(|e| format!("Failed to parse TOML specification: {}", e))?;

    if spec.steps.is_empty() {
        return Err("No steps specified in the pipeline".to_string());
    }

    let sorted_steps = topological_sort(&spec.steps)?;
    info!(
        "Topological sort successful. Execution order: {:?}",
        sorted_steps.iter().map(|s| &s.name).collect::<Vec<_>>()
    );

    // Shared chain run ID
    let shared_run_id = uuid::Uuid::new_v4().to_string();
    let proj_name = project.unwrap_or_else(|| "default".to_string());

    // Connect to local StateDb for durable event persistence
    let db_path = home::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".gestalt")
        .join("state.db");

    let db = Arc::new(
        StateDb::open(&db_path)
            .map_err(|e| format!("Failed to open StateDb at {}: {}", db_path.display(), e))?,
    );

    let sink = std::env::var("XAVIER_TOKEN")
        .ok()
        .filter(|t| !t.is_empty())
        .map(|_| gestalt_router::xavier_sink::XavierEventSink::from_env());

    let mut any_step_failed = false;

    for step in sorted_steps {
        info!("Executing step '{}' using agent '{}'", step.name, step.agent);

        // 1. Emit `run_started` event on the universal event bus
        let start_ev = BusEvent::new(
            &step.agent,
            "run_started",
            format!("Step '{}' (agent: '{}') started: {}", step.name, step.agent, step.task),
        )
        .with_run_id(&shared_run_id)
        .with_project(&proj_name)
        .with_state("Running");

        if let Err(e) = handle_event(&db, &start_ev, sink.as_ref()).await {
            warn!("Failed to emit run_started event to bus: {}", e);
        }

        // 2. Set up InMemoryVfs for the agent wrapper
        let vfs = Arc::new(InMemoryVfs::new());
        let command_str = format!("{} {}", step.agent, step.task);

        let wrapper = AgentWrapper::new(
            vfs.clone(),
            step.agent.clone(),
            shared_run_id.clone(),
            command_str,
        );

        // 3. Execute step using AgentWrapper (with captured exit status, stdout, stderr)
        let execution_result = wrapper.execute_with_output().await;

        match execution_result {
            Ok((edits, status, stdout, stderr)) => {
                if status.success() {
                    info!("Step '{}' finished successfully.", step.name);

                    // Emit `run_finished` success event
                    let finish_ev = BusEvent::new(
                        &step.agent,
                        "run_finished",
                        format!("Step '{}' finished successfully", step.name),
                    )
                    .with_run_id(&shared_run_id)
                    .with_project(&proj_name)
                    .with_state("Success");

                    if let Err(e) = handle_event(&db, &finish_ev, sink.as_ref()).await {
                        warn!("Failed to emit run_finished success event to bus: {}", e);
                    }

                    // Append output summary to next step's XAVIER_CONTEXT
                    let summary = build_output_summary(&step.name, &stdout, &edits);
                    append_to_xavier_context(&summary);
                } else {
                    error!(
                        "Step '{}' command failed with non-zero status. Stderr: {}",
                        step.name, stderr
                    );
                    any_step_failed = true;

                    // Emit `run_finished` failed event
                    let fail_ev = BusEvent::new(
                        &step.agent,
                        "run_finished",
                        format!("Step '{}' failed with status: {:?}", step.name, status),
                    )
                    .with_run_id(&shared_run_id)
                    .with_project(&proj_name)
                    .with_state("Crashed");

                    if let Err(e) = handle_event(&db, &fail_ev, sink.as_ref()).await {
                        warn!("Failed to emit run_finished failed event to bus: {}", e);
                    }

                    if !continue_on_error {
                        return Err(format!(
                            "Chain halted. Step '{}' failed with status: {:?}",
                            step.name, status
                        ));
                    }
                }
            }
            Err(err_msg) => {
                error!("Step '{}' execution error: {}", step.name, err_msg);
                any_step_failed = true;

                // Emit failed event on VFS or wrapper execution error
                let fail_ev = BusEvent::new(
                    &step.agent,
                    "run_finished",
                    format!("Step '{}' execution error: {}", step.name, err_msg),
                )
                .with_run_id(&shared_run_id)
                .with_project(&proj_name)
                .with_state("Crashed");

                if let Err(e) = handle_event(&db, &fail_ev, sink.as_ref()).await {
                    warn!("Failed to emit run_finished failed event to bus: {}", e);
                }

                if !continue_on_error {
                    return Err(format!("Chain halted. Step '{}' failed: {}", step.name, err_msg));
                }
            }
        }
    }

    if any_step_failed {
        Err("One or more steps in the chain failed to execute successfully".to_string())
    } else {
        Ok(())
    }
}

/// Helper to format a rich output summary from stdout or parsed edits.
fn build_output_summary(step_name: &str, stdout: &str, edits: &[BlockEdit]) -> String {
    let mut summary = format!("=== Step '{}' Output ===\n", step_name);
    if !stdout.trim().is_empty() {
        summary.push_str(stdout.trim());
        summary.push('\n');
    }

    if !edits.is_empty() {
        summary.push_str("VFS Edits applied:\n");
        for edit in edits {
            match edit {
                BlockEdit::Insert { path, line, .. } => {
                    summary.push_str(&format!(" - Inserted lines in {path} at line {line}\n"));
                }
                BlockEdit::Delete { path, line } => {
                    summary.push_str(&format!(" - Deleted line in {path} at line {line}\n"));
                }
                BlockEdit::Replace { path, line, .. } => {
                    summary.push_str(&format!(" - Replaced content in {path} at line {line}\n"));
                }
            }
        }
    }

    if stdout.trim().is_empty() && edits.is_empty() {
        summary.push_str("Execution completed with no printed output or VFS edits.\n");
    }

    summary
}

/// Safely append an output summary to the XAVIER_CONTEXT JSON list.
fn append_to_xavier_context(summary: &str) {
    let mut context_list: Vec<String> = if let Ok(existing) = std::env::var("XAVIER_CONTEXT") {
        serde_json::from_str(&existing).unwrap_or_else(|_| {
            if !existing.trim().is_empty() {
                vec![existing]
            } else {
                vec![]
            }
        })
    } else {
        vec![]
    };

    context_list.push(summary.to_string());

    if let Ok(new_context) = serde_json::to_string(&context_list) {
        std::env::set_var("XAVIER_CONTEXT", new_context);
    }
}
