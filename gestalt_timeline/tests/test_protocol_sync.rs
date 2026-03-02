use gestalt_timeline::db::SurrealClient;
use gestalt_timeline::models::{Project, Task, TaskStatus};
use gestalt_timeline::services::{ProtocolSyncService, TimelineService};
use std::path::Path;
use tempfile::tempdir;
use tokio::fs;

#[tokio::test]
async fn test_protocol_sync_two_way() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let task_md_path = dir.path().join("TASK.md");

    let initial_content = r#"
# 📋 TASK.md

| ID | Task | Status |
|----|------|--------|
| T-01 | Fix auth bug | ⏳ Pending |
| T-02 | Add tests | ✅ Completed |
"#;
    fs::write(&task_md_path, initial_content).await?;

    let db = SurrealClient::connect_mem().await?;
    let timeline = TimelineService::new(db.clone());
    let sync_service = ProtocolSyncService::new(db.clone(), timeline.clone());

    // Create project
    let project = Project::new("test-project", "tester");
    db.create("projects", &project).await?;

    // 1. Sync from markdown to DB
    sync_service.sync_from_markdown(&task_md_path, "test-project", "tester").await?;

    // Verify tasks in DB
    let tasks: Vec<Task> = db.select_all("tasks").await?;
    println!("Tasks in DB: {:?}", tasks);
    assert_eq!(tasks.len(), 2);

    let t1 = tasks.iter().find(|t| t.external_id.as_deref() == Some("T-01")).unwrap();
    assert_eq!(t1.status, TaskStatus::Pending);
    assert_eq!(t1.description, "Fix auth bug");

    let t2 = tasks.iter().find(|t| t.external_id.as_deref() == Some("T-02")).unwrap();
    assert_eq!(t2.status, TaskStatus::Completed);

    // 2. Update task in DB
    let t1_id = match &t1.id.as_ref().unwrap().id {
        surrealdb::sql::Id::String(s) => s.clone(),
        id => id.to_string(),
    };
    let mut t1_updated = t1.clone();
    t1_updated.status = TaskStatus::Running;
    db.update("tasks", &t1_id, &t1_updated).await?;

    // 3. Sync from DB to markdown
    sync_service.sync_to_markdown(&task_md_path, "test-project").await?;

    // Verify markdown content
    let updated_content = fs::read_to_string(&task_md_path).await?;
    assert!(updated_content.contains("| T-01 | Fix auth bug | 🔄 Running |"));
    assert!(updated_content.contains("| T-02 | Add tests | ✅ Completed |"));

    Ok(())
}
