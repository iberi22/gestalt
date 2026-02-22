//! Integration tests for Gestalt Timeline CLI parsing.

use clap::Parser;
use gestalt_timeline::cli::{AgentCommands, Cli, Commands};

#[test]
fn test_main_parsing_with_global_flags() {
    let cli = Cli::try_parse_from(["gestalt", "--json", "--context", "--mode", "build"]).unwrap();
    assert!(cli.json);
    assert!(cli.context);
    assert_eq!(cli.mode, "build");
    assert!(cli.command.is_none());
}

#[test]
fn test_add_project_parsing() {
    let cli = Cli::try_parse_from(["gestalt", "add-project", "my-proj"]).unwrap();
    match cli.command {
        Some(Commands::AddProject { name }) => assert_eq!(name, "my-proj"),
        _ => panic!("Expected add-project command"),
    }
}

#[test]
fn test_add_task_parsing() {
    let cli = Cli::try_parse_from(["gestalt", "add-task", "my-proj", "do work"]).unwrap();
    match cli.command {
        Some(Commands::AddTask { project, description }) => {
            assert_eq!(project, "my-proj");
            assert_eq!(description, "do work");
        }
        _ => panic!("Expected add-task command"),
    }
}

#[test]
fn test_watch_parsing() {
    let cli = Cli::try_parse_from([
        "gestalt",
        "watch",
        "--project",
        "proj-1",
        "--events",
        "TaskCreated,TaskCompleted",
    ])
    .unwrap();
    match cli.command {
        Some(Commands::Watch { project, events }) => {
            assert_eq!(project.as_deref(), Some("proj-1"));
            assert_eq!(events.as_deref(), Some("TaskCreated,TaskCompleted"));
        }
        _ => panic!("Expected watch command"),
    }
}

#[test]
fn test_broadcast_parsing() {
    let cli = Cli::try_parse_from(["gestalt", "broadcast", "hello", "--project", "proj-1"]).unwrap();
    match cli.command {
        Some(Commands::Broadcast { message, project }) => {
            assert_eq!(message, "hello");
            assert_eq!(project.as_deref(), Some("proj-1"));
        }
        _ => panic!("Expected broadcast command"),
    }
}

#[test]
fn test_timeline_parsing() {
    let cli = Cli::try_parse_from(["gestalt", "timeline", "--since", "1h"]).unwrap();
    match cli.command {
        Some(Commands::Timeline { since }) => assert_eq!(since.as_deref(), Some("1h")),
        _ => panic!("Expected timeline command"),
    }
}

#[test]
fn test_agent_connect_parsing() {
    let cli = Cli::try_parse_from(["gestalt", "agent-connect", "--name", "worker-a"]).unwrap();
    match cli.command {
        Some(Commands::AgentConnect { name }) => assert_eq!(name.as_deref(), Some("worker-a")),
        _ => panic!("Expected agent-connect command"),
    }
}

#[test]
fn test_list_agents_parsing() {
    let cli = Cli::try_parse_from(["gestalt", "list-agents", "--online"]).unwrap();
    match cli.command {
        Some(Commands::ListAgents { online }) => assert!(online),
        _ => panic!("Expected list-agents command"),
    }
}

#[test]
fn test_subscribe_parsing() {
    let cli = Cli::try_parse_from(["gestalt", "subscribe", "proj-1"]).unwrap();
    match cli.command {
        Some(Commands::Subscribe { project }) => assert_eq!(project, "proj-1"),
        _ => panic!("Expected subscribe command"),
    }
}

#[test]
fn test_agent_spawn_subcommand_parsing() {
    let cli = Cli::try_parse_from(["gestalt", "agent", "spawn", "codex", "fix tests"]).unwrap();
    match cli.command {
        Some(Commands::Agent { action }) => match action {
            AgentCommands::Spawn { agent_type, prompt } => {
                assert_eq!(agent_type, "codex");
                assert_eq!(prompt, "fix tests");
            }
            _ => panic!("Expected agent spawn subcommand"),
        },
        _ => panic!("Expected agent command"),
    }
}
