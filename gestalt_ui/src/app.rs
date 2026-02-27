use std::sync::Arc;
use tokio::runtime::Runtime;
use eframe::egui;
use gestalt_timeline::models::TimelineEvent;
use chrono::{DateTime, Utc};

pub struct GestaltApp {
    pub rt: Arc<Runtime>,
    pub events: Vec<TimelineEvent>,
    pub agents: Vec<String>,
    pub last_update: DateTime<Utc>,
    pub is_loading: bool,
    pub zoom_level: f32,
    pub scroll_offset: f32,
    pub command_input: String,
    pub show_command_node: bool,
    pub selected_event: Option<usize>, // Index of selected event in mock data
}

impl GestaltApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, rt: Arc<Runtime>) -> Self {
        Self {
            rt,
            events: Vec::new(),
            agents: Vec::new(),
            last_update: Utc::now(),
            is_loading: false,
            zoom_level: 1.0,
            scroll_offset: 0.0,
            command_input: String::new(),
            show_command_node: true,
            selected_event: None,
        }
    }

    pub fn init(&mut self) {
        // Here we would spawn a background task to fetch events from SurrealDB
        // For the MVP, let's add some mock data if DB is empty
        if self.events.is_empty() {
             self.add_mock_data();
        }
    }

    fn add_mock_data(&mut self) {
        // Mocking some agents and events
        self.agents = vec![
            "gestalt-orchestrator".to_string(),
            "rust-specialist".to_string(),
            "ui-designer".to_string(),
        ];

        // Mock events would go here
    }
}

impl eframe::App for GestaltApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Gestalt Vision - Universal Timeline");

            ui.separator();

            // Render basic timeline container
            egui::ScrollArea::both().show(ui, |ui| {
                crate::views::timeline::render_timeline(ui, self);
            });

            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("Last updated: {}", self.last_update.format("%H:%M:%S")));
                    if ui.button("Refresh").clicked() {
                        self.init();
                    }
                });
            });
        });

        // Request a repaint if there's ongoing work or for animations
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::runtime::Runtime;

    fn test_app() -> GestaltApp {
        let rt = Arc::new(Runtime::new().unwrap());
        // eframe::CreationContext is tricky to mock without a window,
        // so we test the core logic of app.init() and state manipulations directly
        // by constructing it without a full creation context if possible.
        // For MVP, we'll initialize manually.
        GestaltApp {
            rt,
            events: Vec::new(),
            agents: Vec::new(),
            last_update: Utc::now(),
            is_loading: false,
            zoom_level: 1.0,
            scroll_offset: 0.0,
            command_input: String::new(),
            show_command_node: true,
            selected_event: None,
        }
    }

    #[test]
    fn test_app_initialization() {
        let mut app = test_app();

        // Before init, vectors should be empty
        assert!(app.agents.is_empty());
        assert!(app.events.is_empty());

        // Initialize mock data
        app.init();

        // After init, agents should be populated via mock data
        assert!(!app.agents.is_empty());
        assert_eq!(app.agents.len(), 3);
        assert_eq!(app.agents[0], "gestalt-orchestrator");
    }

    #[test]
    fn test_app_ui_state_defaults() {
        let app = test_app();

        // Verify defaults for the futuristic UI
        assert_eq!(app.zoom_level, 1.0);
        assert_eq!(app.scroll_offset, 0.0);
        assert!(app.show_command_node); // Command Node should be visible by default
        assert!(app.selected_event.is_none()); // No event inspected by default
        assert!(app.command_input.is_empty()); // Input buffer should be empty
    }
}
