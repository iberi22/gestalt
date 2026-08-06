//! OpenTelemetry GenAI Interop mapping for universal `BusEvent`.
//!
//! Maps a `BusEvent` to OpenTelemetry semantic convention metadata (`metadata.otel`).

use crate::event_bus::BusEvent;
use serde_json::{Map, Value};

/// Maps a [`BusEvent`] to an OpenTelemetry-compatible attributes JSON representation.
///
/// Under Wave 8 FEAT-GT-034, this handles mapping of:
/// - `run_id` → `"gen_ai.conversation.id"` & `"trace_id"`
/// - `event_type` → `"span_type"` (e.g. `run_started` -> `invoke_agent`, `checkpoint` -> `execute_tool`, else -> `chat`)
/// - `agent` → `"gen_ai.agent.name"`
/// - `metadata.llm` → `"gen_ai.request.model"`
/// - `metadata.provider` → `"gen_ai.system"`
/// - Other `metadata` fields prefixed with `"gen_ai."`
pub fn bus_event_to_otel_attributes(event: &BusEvent) -> Value {
    let mut attrs = Map::new();

    // Map trace_id and gen_ai.conversation.id if run_id is present
    if let Some(ref run_id) = event.run_id {
        attrs.insert("trace_id".to_string(), Value::String(run_id.clone()));
        attrs.insert("gen_ai.conversation.id".to_string(), Value::String(run_id.clone()));
    }

    // Map span_type
    let span_type = match event.event_type.as_str() {
        "run_started" => "invoke_agent",
        "checkpoint" => "execute_tool",
        _ => "chat",
    };
    attrs.insert("span_type".to_string(), Value::String(span_type.to_string()));

    // Map agent
    attrs.insert("gen_ai.agent.name".to_string(), Value::String(event.agent.clone()));

    // Map other standard BusEvent fields to semantic convention attributes if useful
    if let Some(ref project) = event.project {
        attrs.insert("gen_ai.project".to_string(), Value::String(project.clone()));
    }
    if let Some(ref state) = event.state {
        attrs.insert("gen_ai.state".to_string(), Value::String(state.clone()));
    }
    attrs.insert("gen_ai.event.summary".to_string(), Value::String(event.summary.clone()));
    attrs.insert("gen_ai.event.ts".to_string(), Value::String(event.ts.clone()));

    // Map metadata to gen_ai.* attributes
    if let Value::Object(ref meta_obj) = event.metadata {
        for (key, val) in meta_obj {
            if key == "llm" || key.starts_with("llm") {
                attrs.insert("gen_ai.request.model".to_string(), val.clone());
            } else if key == "provider" {
                attrs.insert("gen_ai.system".to_string(), val.clone());
            } else {
                let new_key = if key.starts_with("gen_ai.") {
                    key.clone()
                } else {
                    format!("gen_ai.{}", key)
                };
                attrs.insert(new_key, val.clone());
            }
        }
    }

    Value::Object(attrs)
}
