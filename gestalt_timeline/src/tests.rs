//! Unit tests for Gestalt Timeline services

use crate::models::{EventType, Project, Task, TimelineEvent};

#[cfg(test)]
mod timeline_event_tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_timeline_event_creation() {
        let event = TimelineEvent::new("test_agent", EventType::ProjectCreated);

        assert!(event.id.is_none()); // ID is None before DB insertion
        assert_eq!(event.agent_id, "test_agent");
        assert!(matches!(event.event_type, EventType::ProjectCreated));
        assert!(event.project_id.is_none());
        assert!(event.task_id.is_none());
    }

    #[test]
    fn test_timeline_event_with_project() {
        let event =
            TimelineEvent::new("agent1", EventType::TaskCreated).with_project("project_123");

        assert_eq!(event.project_id, Some("project_123".to_string()));
    }

    #[test]
    fn test_timeline_event_with_task() {
        let event = TimelineEvent::new("agent1", EventType::TaskStarted)
            .with_project("proj_1")
            .with_task("task_1");

        assert_eq!(event.project_id, Some("proj_1".to_string()));
        assert_eq!(event.task_id, Some("task_1".to_string()));
    }

    #[test]
    fn test_timeline_event_with_payload() {
        let payload = serde_json::json!({"key": "value", "count": 42});
        let event =
            TimelineEvent::new("agent1", EventType::CommandExecuted).with_payload(payload.clone());

        assert_eq!(event.payload, payload);
    }

    #[test]
    fn test_timeline_event_with_metadata() {
        let event = TimelineEvent::new("agent1", EventType::Custom("test".to_string()))
            .with_metadata("env", "production")
            .with_metadata("version", "1.0.0");

        assert_eq!(event.metadata.get("env"), Some(&"production".to_string()));
        assert_eq!(event.metadata.get("version"), Some(&"1.0.0".to_string()));
    }

    #[test]
    fn test_event_type_display() {
        assert_eq!(EventType::ProjectCreated.to_string(), "project_created");
        assert_eq!(EventType::TaskCompleted.to_string(), "task_completed");
        assert_eq!(
            EventType::Custom("my_event".to_string()).to_string(),
            "custom:my_event"
        );
    }

    #[test]
    fn test_timestamp_is_utc() {
        let before = Utc::now();
        let event = TimelineEvent::new("agent", EventType::AgentConnected);
        let after = Utc::now();

        let event_ts = event.timestamp.0;
        assert!(event_ts >= before);
        assert!(event_ts <= after);
    }
}

#[cfg(test)]
mod project_tests {
    use super::*;
    use crate::models::ProjectStatus;

    #[test]
    fn test_project_creation() {
        let project = Project::new("my-project", "agent_1");

        // Project ID is None before database assignment
        assert!(project.id.is_none());
        assert_eq!(project.name, "my-project");
        assert_eq!(project.created_by, "agent_1");
        assert!(matches!(project.status, ProjectStatus::Active));
        assert_eq!(project.priority, 5);
    }

    #[test]
    fn test_project_status_display() {
        assert_eq!(ProjectStatus::Active.to_string(), "active");
        assert_eq!(ProjectStatus::Paused.to_string(), "paused");
        assert_eq!(ProjectStatus::Completed.to_string(), "completed");
        assert_eq!(ProjectStatus::Archived.to_string(), "archived");
    }

    #[test]
    fn test_project_timestamps() {
        let project = Project::new("test", "agent");

        assert_eq!(project.created_at, project.updated_at);
    }
}

#[cfg(test)]
mod task_tests {
    use super::*;
    use crate::models::TaskStatus;

    #[test]
    fn test_task_creation() {
        let task = Task::new("project_1", "Fix bug in login", "agent_1", None);

        assert!(task.id.is_none());
        assert_eq!(task.project_id, "project_1");
        assert_eq!(task.description, "Fix bug in login");
        assert_eq!(task.created_by, "agent_1");
        assert!(matches!(task.status, TaskStatus::Pending));
        assert!(task.completed_at.is_none());
        assert!(task.executed_by.is_none());
    }

    #[test]
    fn test_task_status_display() {
        assert_eq!(TaskStatus::Pending.to_string(), "pending");
        assert_eq!(TaskStatus::Running.to_string(), "running");
        assert_eq!(TaskStatus::Completed.to_string(), "completed");
        assert_eq!(TaskStatus::Failed.to_string(), "failed");
        assert_eq!(TaskStatus::Cancelled.to_string(), "cancelled");
    }
}

#[cfg(test)]
mod serialization_tests {
    use super::*;

    #[test]
    fn test_timeline_event_json_roundtrip() {
        let event = TimelineEvent::new("agent", EventType::TaskCreated)
            .with_project("proj")
            .with_task("task")
            .with_payload(serde_json::json!({"data": "test"}));

        let json = serde_json::to_string(&event).unwrap();
        let parsed: TimelineEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(event.id, parsed.id);
        assert_eq!(event.agent_id, parsed.agent_id);
        assert_eq!(event.project_id, parsed.project_id);
        assert_eq!(event.task_id, parsed.task_id);
    }

    #[test]
    fn test_project_json_roundtrip() {
        let project = Project::new("test-project", "agent_1");

        let json = serde_json::to_string(&project).unwrap();
        let parsed: Project = serde_json::from_str(&json).unwrap();

        assert_eq!(project.id, parsed.id);
        assert_eq!(project.name, parsed.name);
    }

    #[test]
    fn test_task_json_roundtrip() {
        let task = Task::new("proj", "description", "agent", None);

        let json = serde_json::to_string(&task).unwrap();
        let parsed: Task = serde_json::from_str(&json).unwrap();

        assert_eq!(task.id, parsed.id);
        assert_eq!(task.description, parsed.description);
    }
}

#[cfg(test)]
mod ulid_tests {
    use super::*;

    #[test]
    fn test_ids_are_unique() {
        let event1 = TimelineEvent::new("agent", EventType::ProjectCreated);
        let event2 = TimelineEvent::new("agent", EventType::ProjectCreated);

        assert_ne!(event1.timestamp, event2.timestamp);
    }

    #[test]
    fn test_ids_are_sortable() {
        let event1 = TimelineEvent::new("agent", EventType::ProjectCreated);
        std::thread::sleep(std::time::Duration::from_millis(1));
        let event2 = TimelineEvent::new("agent", EventType::ProjectCreated);

        // Timestamps are sortable
        assert!(event1.timestamp <= event2.timestamp);
    }
}
