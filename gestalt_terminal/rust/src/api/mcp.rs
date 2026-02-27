use crate::frb_generated::StreamSink;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use std::sync::{Mutex, LazyLock};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum McpComponent {
    Card { title: String, content: String },
    Button { label: String, action_id: String },
    Markdown { content: String },
    Row { children: Vec<McpComponent> },
    Column { children: Vec<McpComponent> },
    Image { url: String, alt: String },
    ProgressBar { progress: f64, label: String },
    Input { label: String, field_id: String },
}

static MCP_SINK: LazyLock<Mutex<Option<StreamSink<McpComponent>>>> = LazyLock::new(|| Mutex::new(None));

pub fn stream_mcp_ui(sink: StreamSink<McpComponent>) -> Result<()> {
    {
        let mut guard = MCP_SINK.lock().unwrap();
        *guard = Some(sink.clone());
    }

    // Send initial state
    sink.add(McpComponent::Card {
        title: "Gestalt Agent".into(),
        content: "System Initialized. Waiting for events...".into(),
    }).ok();

    Ok(())
}

#[flutter_rust_bridge::frb(sync)]
pub fn simulate_agent_event(event_type: String) -> Result<()> {
    let guard = MCP_SINK.lock().unwrap();
    if let Some(sink) = guard.as_ref() {
        let component = match event_type.as_str() {
             "analysis" => McpComponent::Column {
                children: vec![
                    McpComponent::Image {
                        url: "assets/crab_icon.png".into(),
                        alt: "Rust Crab".into(),
                    },
                    McpComponent::Markdown {
                        content: "# System Analysis\n\nRunning deep diagnostics...".into()
                    },
                    McpComponent::ProgressBar { progress: 0.45, label: "Scanning Memory...".into() },
                ]
            },
            "action" => McpComponent::Row {
                children: vec![
                    McpComponent::Input { label: "Reason for approval".into(), field_id: "reason".into() },
                    McpComponent::Button { label: "Approve".into(), action_id: "approve".into() },
                    McpComponent::Button { label: "Reject".into(), action_id: "reject".into() },
                ]
            },
            "result" => McpComponent::Card {
                title: "Operation Complete".into(),
                content: "The operation completed successfully.".into(),
            },
             _ => McpComponent::Card { title: "Event".into(), content: event_type },
        };
        sink.add(component).ok();
    }
    Ok(())
}

#[flutter_rust_bridge::frb(sync)]
pub fn handle_mcp_action(action_id: String, value: String) -> Result<()> {
    println!("Action Received: {} with value: {}", action_id, value);
    // In a real agent, this would trigger a state change or new LLM prompt.
    // For now, we'll just echo it back via the stream if possible, or just log.

    let guard = MCP_SINK.lock().unwrap();
    if let Some(sink) = guard.as_ref() {
        sink.add(McpComponent::Card {
            title: "Action Acknowledged".into(),
            content: format!("Received action '{}'. Processing...", action_id),
        }).ok();
    }
    Ok(())
}
