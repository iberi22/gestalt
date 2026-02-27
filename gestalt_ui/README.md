# Gestalt Vision - Command Deck UI (`gestalt_ui`)

This crate provides a high-performance, native Rust visualization interface for the Gestalt agent system. Powered by `egui` and `eframe`, it serves as a "Command Deck", running alongside the standard Flutter application (`gestalt_app`) to offer an immersive, real-time look into the system's timeline and communication bus.

## Features

- **The Universal Timeline Canvas:** A deeply dark, holographic canvas with a subtle grid that visualizes the passage of time and agent actions.
- **Agent Lanes:** Horizontal tracks dynamically generated for each active agent (e.g., `CORE_ORCHESTRATOR`, `RUST_SPECIALIST`), making parallel execution visually comprehensible.
- **The Gestalt Data Bus:** A neon cyan pulse-line at the bottom of the interface. Event nodes drop signal lines to the bus, illustrating systemic data flow.
- **Floating Command Node:** Chat and command inputs are handled via a translucent, draggable window that hovers over the timeline, complete with execution history.
- **Interactive Inspect Nodes:** Clicking on any event block in the timeline spawns a floating popover displaying the exact JSON payload or reasoning trace of that event.

## Architecture

This is an immediate-mode GUI built completely in Rust:
- **Zero FFI overhead**: Directly consumes the `gestalt_timeline` core domains.
- **`app.rs`**: Manages the overarching state, including floating window positions, the currently selected event for inspection, and the mock/live data buffers.
- **`views/timeline.rs`**: Contains the highly customized `egui::Painter` logic to draw the grid, lanes, event blocks, and glowing bus connections.

## Development & Usage

### Running the UI

Since this is a standard Rust application within the workspace, you can compile and launch it directly:

```powershell
cargo run -p gestalt_ui
```

### Running Tests

To verify the integrity of the application state and core rendering math logic, run:

```powershell
cargo test -p gestalt_ui
```

## Future Integration

Currently, the UI displays rich mock data to demonstrate its structural capabilities. The next phase will involve hooking the `GestaltApp::init` state directly into the `gestalt_timeline::db::surreal::SurrealClient` to stream live `TimelineEvent` structs into the visualization engine.
