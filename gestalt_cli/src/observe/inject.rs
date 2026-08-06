use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

/// Safe-merge Codex hooks while preserving existing structures and keys, specifically avoiding duplicate hooks.
pub fn merge_codex_hooks(existing: &str, new_hook_json: &str) -> String {
    // Parse existing JSON or default to a clean object
    let mut root: Value = serde_json::from_str(existing).unwrap_or_else(|_| json!({}));

    // Ensure we have a top-level "hooks" object
    if !root.is_object() {
        root = json!({"hooks": {}});
    } else if root.get("hooks").is_none() || !root["hooks"].is_object() {
        root.as_object_mut()
            .unwrap()
            .insert("hooks".to_string(), json!({}));
    }

    let hooks_obj = root["hooks"].as_object_mut().unwrap();

    // Parse the new hook JSON
    let new_hook: Value = serde_json::from_str(new_hook_json).unwrap_or_else(|_| {
        // If not valid JSON, treat it as a command string and construct the hook structure
        json!({
            "type": "command",
            "command": new_hook_json,
            "timeout": 10
        })
    });

    // Merge the hook under standard Codex event keys
    for event_key in &["SessionStart", "UserPromptSubmit"] {
        let entry = hooks_obj
            .entry(event_key.to_string())
            .or_insert_with(|| json!([]));
        if !entry.is_array() {
            *entry = json!([]);
        }
        let arr = entry.as_array_mut().unwrap();

        // Codex/Orca structure: event_key: [{ "hooks": [{type: "command", command: "...", timeout: 10}] }]
        // If the array is empty, we initialize it with a single group { "hooks": [] }
        if arr.is_empty() {
            arr.push(json!({ "hooks": [] }));
        }

        let mut appended = false;
        for group in arr.iter_mut() {
            if let Some(group_obj) = group.as_object_mut() {
                let inner_hooks_entry = group_obj
                    .entry("hooks".to_string())
                    .or_insert_with(|| json!([]));
                if let Some(inner_hooks_arr) = inner_hooks_entry.as_array_mut() {
                    // Check if our hook is already present in this group's hooks array (based on 'command' field)
                    let already_exists = inner_hooks_arr.iter().any(|h| {
                        h.get("command").and_then(|c| c.as_str())
                            == new_hook.get("command").and_then(|c| c.as_str())
                    });

                    if !already_exists {
                        inner_hooks_arr.push(new_hook.clone());
                        appended = true;
                        break;
                    } else {
                        // Already exists, don't add duplicate
                        appended = true;
                        break;
                    }
                }
            }
        }

        // If not appended yet (e.g. elements in the array weren't objects with "hooks"), push a new group
        if !appended {
            arr.push(json!({
                "hooks": [new_hook.clone()]
            }));
        }
    }

    serde_json::to_string_pretty(&root).unwrap_or_else(|_| existing.to_string())
}

/// Safe-merge Claude settings while preserving existing keys.
pub fn merge_claude_settings(existing: &str, new_hook_cmd: &str) -> String {
    let mut root: Value = serde_json::from_str(existing).unwrap_or_else(|_| json!({}));

    if !root.is_object() {
        root = json!({});
    }

    let root_obj = root.as_object_mut().unwrap();

    // Merge into a "hooks" field at the top-level
    let hooks_obj = root_obj
        .entry("hooks".to_string())
        .or_insert_with(|| json!({}));
    if !hooks_obj.is_object() {
        *hooks_obj = json!({});
    }

    let hooks_map = hooks_obj.as_object_mut().unwrap();

    // Use a "post_run" hook list
    let entry = hooks_map
        .entry("post_run".to_string())
        .or_insert_with(|| json!([]));
    if !entry.is_array() {
        *entry = json!([]);
    }
    let arr = entry.as_array_mut().unwrap();

    let already_exists = arr.iter().any(|v| v.as_str() == Some(new_hook_cmd));
    if !already_exists {
        arr.push(Value::String(new_hook_cmd.to_string()));
    }

    serde_json::to_string_pretty(&root).unwrap_or_else(|_| existing.to_string())
}

/// Dynamically builds the observation hook script with replaced agent, event, and summary values.
pub fn build_hook_script(agent: &str, event_type: &str, summary: &str) -> String {
    let base_script = include_str!("assets/gestalt-observe-hook.sh");
    base_script
        .replace("unknown-agent", agent)
        .replace("run_started", event_type)
        .replace("Agent execution event", summary)
}

/// Inject the Opencode status plugin JS into the given config directory.
pub fn inject_opencode_plugin(config_dir: &Path) -> std::io::Result<PathBuf> {
    let plugin_path = config_dir.join("plugins").join("gestalt-observe-status.js");
    if let Some(parent) = plugin_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = include_str!("assets/gestalt-observe-status.js");
    fs::write(&plugin_path, content)?;
    Ok(plugin_path)
}

/// Inject the Codex hooks into hooks.json under the given directory.
pub fn inject_codex_hooks(codex_dir: &Path, new_hook_cmd: &str) -> std::io::Result<PathBuf> {
    fs::create_dir_all(codex_dir)?;
    let hooks_file = codex_dir.join("hooks.json");
    let existing = if hooks_file.exists() {
        fs::read_to_string(&hooks_file)?
    } else {
        String::new()
    };
    let merged = merge_codex_hooks(&existing, new_hook_cmd);
    fs::write(&hooks_file, merged)?;
    Ok(hooks_file)
}

/// Inject the Claude settings into settings.json under the given directory.
pub fn inject_claude_settings(claude_dir: &Path, new_hook_cmd: &str) -> std::io::Result<PathBuf> {
    fs::create_dir_all(claude_dir)?;
    let settings_file = claude_dir.join("settings.json");
    let existing = if settings_file.exists() {
        fs::read_to_string(&settings_file)?
    } else {
        String::new()
    };
    let merged = merge_claude_settings(&existing, new_hook_cmd);
    fs::write(&settings_file, merged)?;
    Ok(settings_file)
}
