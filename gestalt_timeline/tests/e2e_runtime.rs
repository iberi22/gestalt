use anyhow::Result;
use gestalt_core::application::agent::tools::{ExecuteShellTool, ReadFileTool, WriteFileTool};
use gestalt_timeline::config::DatabaseSettings;
use gestalt_timeline::db::SurrealClient;
use gestalt_timeline::services::{
    AgentRuntime, AgentService, ProjectService, TaskService, TimelineService, WatchService,
};
use std::sync::Arc;
use synapse_agentic::prelude::{DecisionEngine, EmptyContext, ToolRegistry};

async fn init_tool_registry() -> Arc<ToolRegistry> {
    let registry = Arc::new(ToolRegistry::new());
    registry.register_tool(ExecuteShellTool).await;
    registry.register_tool(ReadFileTool).await;
    registry.register_tool(WriteFileTool).await;
    registry
}

async fn init_services() -> Result<(ProjectService, TaskService, WatchService, AgentService)> {
    let db_settings = DatabaseSettings {
        url: "mem://".to_string(),
        user: "root".to_string(),
        pass: "root".to_string(),
        namespace: "test".to_string(),
        database: "test".to_string(),
    };
    let db = SurrealClient::connect(&db_settings).await?;
    let timeline = TimelineService::new(db.clone());
    let project_service = ProjectService::new(db.clone(), timeline.clone());
    let task_service = TaskService::new(db.clone(), timeline.clone());
    let watch_service = WatchService::new(db.clone(), timeline.clone());
    let agent_service = AgentService::new(db.clone(), timeline);
    Ok((project_service, task_service, watch_service, agent_service))
}

#[tokio::test]
async fn test_tools_execute_shell_and_read_file() -> Result<()> {
    let registry = init_tool_registry().await;
    let tmp_file = format!("target/runtime-e2e-{}.txt", uuid::Uuid::new_v4());
    let payload = "hello from runtime e2e";

    let write_result = registry
        .call(
            "write_file",
            &EmptyContext,
            serde_json::json!({ "path": tmp_file, "content": payload }),
        )
        .await?;
    assert_eq!(write_result["success"], serde_json::json!(true));

    let read_result = registry
        .call(
            "read_file",
            &EmptyContext,
            serde_json::json!({ "path": tmp_file }),
        )
        .await?;
    assert_eq!(read_result["content"], serde_json::json!(payload));

    #[cfg(target_os = "windows")]
    let cmd = "Write-Output runtime_ok";
    #[cfg(not(target_os = "windows"))]
    let cmd = "echo runtime_ok";

    let shell_result = registry
        .call(
            "execute_shell",
            &EmptyContext,
            serde_json::json!({ "command": cmd }),
        )
        .await?;
    assert_eq!(shell_result["exit_code"], serde_json::json!(0));
    let stdout = shell_result["stdout"].as_str().unwrap_or_default();
    assert!(stdout.contains("runtime_ok"));

    Ok(())
}

#[tokio::test]
async fn test_runtime_loop_starts_with_registry() -> Result<()> {
    let (project_service, task_service, watch_service, agent_service) = init_services().await?;
    let engine = Arc::new(DecisionEngine::new());
    let registry = init_tool_registry().await;

    let runtime = AgentRuntime::new(
        "runtime-test-agent".to_string(),
        engine,
        registry,
        project_service,
        task_service,
        watch_service,
        agent_service,
    );

    runtime.run_loop("Validate runtime bootstrap").await?;
    Ok(())
}
