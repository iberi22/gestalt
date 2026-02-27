use eframe::egui;
use std::sync::Arc;
use tokio::runtime::Runtime;

mod app;
mod views;

use crate::app::GestaltApp;

fn main() -> eframe::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Create a tokio runtime for background tasks
    let rt = Arc::new(Runtime::new().expect("Failed to create Tokio runtime"));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "Gestalt - Universal Timeline",
        options,
        Box::new(|cc| {
            // Restore state if needed
            let mut app = GestaltApp::new(cc, rt);
            app.init();
            Ok(Box::new(app))
        }),
    )
}
