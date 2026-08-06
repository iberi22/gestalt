#[path = "../src/observe/inject.rs"]
pub mod inject;

use std::fs;

#[test]
fn test_merge_real_orca_hooks() {
    let orca_hooks_json = r#"{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/bin/sh -c \"if [ -f ~/.orca/agent-hooks/codex-hook.sh ] && [ -r ~/.orca/agent-hooks/codex-hook.sh ] && [ -x ~/.orca/agent-hooks/codex-hook.sh ]; then ~/.orca/agent-hooks/codex-hook.sh; else cat >/dev/null 2>&1 || :; fi\"",
            "timeout": 10
          }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/bin/sh -c \"if [ -f ~/.orca/agent-hooks/codex-hook.sh ] && [ -r ~/.orca/agent-hooks/codex-hook.sh ] && [ -x ~/.orca/agent-hooks/codex-hook.sh ]; then ~/.orca/agent-hooks/codex-hook.sh; else cat >/dev/null 2>&1 || :; fi\"",
            "timeout": 10
          }
        ]
      }
    ]
  }
}"#;

    let our_hook_cmd = "curl -s -X POST http://127.0.0.1:8081/api/event";

    // Merge our hook
    let merged = inject::merge_codex_hooks(orca_hooks_json, our_hook_cmd);
    let parsed: serde_json::Value = serde_json::from_str(&merged).expect("Failed to parse merged json");

    // Verify top-level structure
    assert!(parsed.is_object());
    let hooks_obj = parsed.get("hooks").and_then(|h| h.as_object()).expect("hooks should be an object");

    // Verify SessionStart and UserPromptSubmit exist
    for key in &["SessionStart", "UserPromptSubmit"] {
        let list = hooks_obj.get(*key).and_then(|v| v.as_array()).expect("event key should be an array");
        assert_eq!(list.len(), 1);
        let group = list[0].as_object().expect("group should be an object");
        let inner_hooks = group.get("hooks").and_then(|v| v.as_array()).expect("inner hooks should be an array");

        // Should now have 2 hooks: Orca's and ours
        assert_eq!(inner_hooks.len(), 2, "Expected exactly 2 hooks in the group (Orca's + ours)");

        // First hook must be Orca's
        let first_cmd = inner_hooks[0].get("command").and_then(|c| c.as_str()).unwrap();
        assert!(first_cmd.contains("orca"));

        // Second hook must be ours
        let second_cmd = inner_hooks[1].get("command").and_then(|c| c.as_str()).unwrap();
        assert_eq!(second_cmd, our_hook_cmd);
    }
}

#[test]
fn test_merge_codex_hooks_idempotent() {
    let empty_json = "{}";
    let hook_cmd = "curl -s -X POST http://127.0.0.1:8081/api/event";

    // First merge
    let merged_once = inject::merge_codex_hooks(empty_json, hook_cmd);
    let parsed_once: serde_json::Value = serde_json::from_str(&merged_once).unwrap();
    let hooks_once = parsed_once["hooks"]["SessionStart"][0]["hooks"].as_array().unwrap();
    assert_eq!(hooks_once.len(), 1);

    // Second merge with same command
    let merged_twice = inject::merge_codex_hooks(&merged_once, hook_cmd);
    let parsed_twice: serde_json::Value = serde_json::from_str(&merged_twice).unwrap();
    let hooks_twice = parsed_twice["hooks"]["SessionStart"][0]["hooks"].as_array().unwrap();

    // No duplicate hooks should be added
    assert_eq!(hooks_twice.len(), 1, "Idempotency failed: duplicate hook added");
    assert_eq!(hooks_twice[0]["command"].as_str().unwrap(), hook_cmd);
}

#[test]
fn test_build_hook_script_contents() {
    let script = inject::build_hook_script("test-agent", "test-event", "test summary description");

    // Hook script must contain:
    // 1. 8081/api/event
    // 2. timeout <= 10s (max-time 10)
    // 3. fail-open fallback (|| :)
    assert!(script.contains("8081/api/event"), "Script does not contain endpoint 8081/api/event");
    assert!(script.contains("--max-time 10") || script.contains("-m 10") || script.contains("timeout"), "Script does not enforce timeout of 10s");
    assert!(script.contains("|| :") || script.contains("|| true"), "Script does not implement fail-open fallback");
    assert!(script.contains("test-agent"), "Script did not substitute agent name");
    assert!(script.contains("test-event"), "Script did not substitute event type");
    assert!(script.contains("test summary description"), "Script did not substitute summary");
}

#[test]
fn test_injectors_with_temp_dir() {
    let base_path = std::env::temp_dir().join(format!("gestalt-inject-test-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&base_path).expect("Failed to create temp directory");

    // 1. Inject Opencode plugin
    let opencode_dir = base_path.join("opencode");
    let plugin_path = inject::inject_opencode_plugin(&opencode_dir).expect("Failed to inject opencode plugin");
    assert!(plugin_path.exists());
    let plugin_content = fs::read_to_string(&plugin_path).unwrap();
    assert!(plugin_content.contains("8081/api/event"));
    assert!(plugin_content.contains("opencode-plugin"));

    // 2. Inject Codex hooks
    let codex_dir = base_path.join("codex");
    let hooks_file = inject::inject_codex_hooks(&codex_dir, "my-gestalt-hook-command").expect("Failed to inject codex hooks");
    assert!(hooks_file.exists());
    let hooks_content = fs::read_to_string(&hooks_file).unwrap();
    assert!(hooks_content.contains("my-gestalt-hook-command"));

    // Inject same Codex hooks again (idempotent)
    inject::inject_codex_hooks(&codex_dir, "my-gestalt-hook-command").expect("Failed to inject codex hooks again");
    let hooks_content_after = fs::read_to_string(&hooks_file).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&hooks_content_after).unwrap();
    let len = parsed["hooks"]["SessionStart"][0]["hooks"].as_array().unwrap().len();
    assert_eq!(len, 1);

    // 3. Inject Claude settings
    let claude_dir = base_path.join("claude");
    let settings_file = inject::inject_claude_settings(&claude_dir, "my-claude-hook-command").expect("Failed to inject claude settings");
    assert!(settings_file.exists());
    let settings_content = fs::read_to_string(&settings_file).unwrap();
    assert!(settings_content.contains("my-claude-hook-command"));

    // Clean up
    let _ = fs::remove_dir_all(&base_path);
}
