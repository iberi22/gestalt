use crate::app::GestaltApp;
use chrono::Utc;
use eframe::egui;
use eframe::egui::{Color32, Pos2, Rect, Stroke, Vec2};

pub fn render_timeline(ui: &mut egui::Ui, app: &mut GestaltApp) {
    let _ctx = ui.ctx().clone();

    // The entire central panel is our canvas
    let (response, painter) = ui.allocate_painter(
        Vec2::new(ui.available_width(), ui.available_height()),
        egui::Sense::click_and_drag(),
    );

    let rect = response.rect;
    let lane_height = 100.0;
    let time_scale = 10.0 * app.zoom_level; // Pixels per second

    // 1. Futuristic Background Grid & Pulse
    painter.rect_filled(rect, 0.0, Color32::from_rgb(15, 15, 19)); // Deep dark background

    let grid_size = 50.0 * app.zoom_level;
    let mut x_grid = rect.left() - (app.scroll_offset % grid_size);
    while x_grid < rect.right() {
        painter.line_segment(
            [Pos2::new(x_grid, rect.top()), Pos2::new(x_grid, rect.bottom())],
            Stroke::new(1.0, Color32::from_white_alpha(10)),
        );
        x_grid += grid_size;
    }

    let mut y_grid = rect.top();
    while y_grid < rect.bottom() {
        painter.line_segment(
            [Pos2::new(rect.left(), y_grid), Pos2::new(rect.right(), y_grid)],
            Stroke::new(1.0, Color32::from_white_alpha(10)),
        );
        y_grid += grid_size;
    }

    // 2. Draw Agent Lanes (Holographic style)
    for (i, agent) in app.agents.iter().enumerate() {
        let y = rect.top() + 80.0 + (i as f32 * lane_height);
        let lane_rect = Rect::from_min_max(
            Pos2::new(rect.left(), y),
            Pos2::new(rect.right(), y + lane_height),
        );

        // Subtle gradient/transparent background for lane
        painter.rect_filled(lane_rect, 0.0, Color32::from_white_alpha(5));

        // Agent Label with "glow" effect (simple text for now)
        painter.text(
            Pos2::new(rect.left() + 20.0, y + 20.0),
            egui::Align2::LEFT_TOP,
            format!("// NODE: {}", agent.to_uppercase()),
            egui::FontId::monospace(14.0),
            Color32::from_rgb(0, 255, 255), // Neon Cyan
        );

        // Subtle bottom border for lane
        painter.line_segment(
            [Pos2::new(rect.left(), y + lane_height), Pos2::new(rect.right(), y + lane_height)],
            Stroke::new(1.0, Color32::from_rgb(0, 80, 80)),
        );
    }

    // 3. The Gestalt Bus (Neon Bottom Line)
    let bus_y = rect.bottom() - 60.0;

    // Simulate a glowing bus by drawing multiple lines with decreasing opacity/increasing width
    painter.line_segment(
        [Pos2::new(rect.left(), bus_y), Pos2::new(rect.right(), bus_y)],
        Stroke::new(6.0, Color32::from_rgba_premultiplied(0, 255, 255, 50)),
    );
    painter.line_segment(
        [Pos2::new(rect.left(), bus_y), Pos2::new(rect.right(), bus_y)],
        Stroke::new(2.0, Color32::from_rgb(0, 255, 255)),
    );

    painter.text(
        Pos2::new(rect.left() + 20.0, bus_y - 20.0),
        egui::Align2::LEFT_BOTTOM,
        "=== GESTALT DATA BUS ===",
        egui::FontId::monospace(12.0),
        Color32::from_rgb(0, 255, 255),
    );

    // 4. Draw Events & Signals
    let start_time = app.last_update - chrono::Duration::seconds(60);

    // Check for clicks to clear selection
    if response.clicked() {
        app.selected_event = None;
    }

    let mut event_idx = 0;
    for (i, _agent) in app.agents.iter().enumerate() {
        let y = rect.top() + 80.0 + (i as f32 * lane_height);

        // Mock events
        for j in 0..3 {
            let current_idx = event_idx;
            event_idx += 1;

            let event_time = start_time + chrono::Duration::seconds(10 + j * 15);
            let age_secs = (Utc::now() - event_time).num_seconds() as f32;
            let x = rect.right() - (age_secs * time_scale) - app.scroll_offset;

            if x > rect.left() && x < rect.right() {
                let event_rect = Rect::from_min_max(
                    Pos2::new(x, y + 30.0),
                    Pos2::new(x + 120.0, y + 80.0),
                );

                let is_selected = app.selected_event == Some(current_idx);

                let base_color = match j % 3 {
                    0 => Color32::from_rgb(0, 150, 255), // Blue task
                    1 => Color32::from_rgb(255, 0, 255), // Purple LLM
                    _ => Color32::from_rgb(0, 255, 100), // Green Success
                };

                let box_color = if is_selected {
                    base_color
                } else {
                    base_color.gamma_multiply(0.3) // Transparent body
                };

                // Draw Signal line dropping to the bus
                painter.line_segment(
                    [Pos2::new(event_rect.center().x, event_rect.bottom()), Pos2::new(event_rect.center().x, bus_y)],
                    Stroke::new(1.0, base_color.gamma_multiply(0.5)),
                );

                // Draw connection blip on the bus
                painter.circle_filled(Pos2::new(event_rect.center().x, bus_y), 4.0, base_color);

                // Draw Event Box
                painter.rect_filled(event_rect, 2.0, box_color);
                painter.rect_stroke(
                    event_rect,
                    2.0,
                    Stroke::new(1.5, base_color),
                    egui::StrokeKind::Middle
                );

                painter.text(
                    event_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("EVT-{:03}", current_idx),
                    egui::FontId::monospace(12.0),
                    if is_selected { Color32::BLACK } else { Color32::WHITE },
                );

                // Interaction
                let interact_rect = ui.interact(event_rect, ui.id().with(current_idx), egui::Sense::click());
                if interact_rect.clicked() {
                    app.selected_event = Some(current_idx);
                }
            }
        }
    }

    // Timeline Drag / Scroll logic
    if response.dragged() {
        app.scroll_offset -= response.drag_delta().x;
    }

    // 5. Floating Windows (Render on top of painter via separate egui calls)

    // A. Command Node (Floating Input)
    if app.show_command_node {
        let mut open = app.show_command_node;
        egui::Window::new("COMMAND NODE")
            .open(&mut open)
            .frame(egui::Frame::window(ui.style()).fill(Color32::from_rgba_premultiplied(20, 20, 25, 230)).stroke(Stroke::new(1.0, Color32::from_rgb(0, 255, 255))))
            .title_bar(true)
            .collapsible(true)
            .resizable(true)
            .default_size([400.0, 150.0])
            .show(ui.ctx(), |ui| {
                ui.visuals_mut().override_text_color = Some(Color32::WHITE);

                egui::ScrollArea::vertical().max_height(100.0).show(ui, |ui| {
                    ui.label(egui::RichText::new(">> System Online. Ready for directives.").color(Color32::from_rgb(0, 255, 100)).monospace());
                    // Console history would go here
                });

                ui.separator();

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(">").color(Color32::from_rgb(0, 255, 255)).monospace());

                    let text_edit = egui::TextEdit::singleline(&mut app.command_input)
                        .desired_width(ui.available_width() - 60.0)
                        .font(egui::TextStyle::Monospace)
                        .text_color(Color32::from_rgb(0, 255, 255));

                    let response = ui.add(text_edit);

                    if ui.button("EXEC").clicked() || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                        // Handle command execution here
                        println!("Executing: {}", app.command_input);
                        app.command_input.clear();
                        response.request_focus(); // Keep focus after sending
                    }
                });
            });
        app.show_command_node = open;
    }

    // B. Event Inspection Node
    if let Some(idx) = app.selected_event {
        egui::Window::new(format!("INSPECT: EVT-{:03}", idx))
            .frame(egui::Frame::window(ui.style()).fill(Color32::from_rgba_premultiplied(30, 20, 30, 230)).stroke(Stroke::new(1.0, Color32::from_rgb(255, 0, 255))))
            .title_bar(true)
            .collapsible(false)
            .resizable(true)
            .default_width(300.0)
            .show(ui.ctx(), |ui| {
                ui.visuals_mut().override_text_color = Some(Color32::WHITE);
                ui.label(egui::RichText::new("PAYLOAD DETAILS").strong().color(Color32::from_rgb(255, 100, 255)));
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mock_json = r#"{
    "status": "processing",
    "agent": "core_orchestrator",
    "confidence": 0.98,
    "trace": [
        "Analyzed input",
        "Queried VectorDB",
        "Generated response"
    ]
}"#;
                    ui.label(egui::RichText::new(mock_json).monospace().color(Color32::from_rgb(200, 200, 255)));
                });

                if ui.button("Close Inspect (Esc)").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    app.selected_event = None;
                }
            });
    }
}
