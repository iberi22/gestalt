//! Gestalt Swarm TUI — Interactive Terminal UI for Swarm Orchestration
//!
//! Build: cargo build --release -p gestalt_tui
//! Run:   ./target/release/gestalt-tui.exe

mod state;
mod swarm;
mod ui;

use state::{new_shared_state, AppState, SharedState};
use swarm::run_swarm_goal;

use crossterm::{
    cursor, event::{self, DisableBracketedPaste, EnableBracketedPaste, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::Stylize,
    Terminal,
};
use std::io;
use tokio::sync::mpsc;

/// Render loop
fn ui_loop(state: SharedState, _input_rx: mpsc::Receiver<()>) {
    // Setup terminal
    io::stdout().execute(EnterAlternateScreen).unwrap();
    io::stdout().execute(EnableBracketedPaste).unwrap();
    enable_raw_mode().unwrap();

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend).unwrap();

    // Hide cursor
    terminal.backend_mut().execute(cursor::Hide).ok();

    // App state (mutable, kept in sync with shared state for rendering)
    let mut render_state = AppState::new();
    let mut focused_panel = 0; // 0 = input, 1 = agents, 2 = output
    let mut input_buffer = String::new();
    let mut needs_redraw = true;
    let mut should_quit = false;

    // Load initial state
    if let Ok(s) = state.read() {
        render_state = s.clone();
    }

    loop {
        // Draw
        if needs_redraw {
            terminal.draw(|f| {
                let size = f.size();

                let vertical = Layout::new(
                    Direction::Vertical,
                    [
                        Constraint::Length(4), // header
                        Constraint::Min(1),    // body
                        Constraint::Length(5), // input
                        Constraint::Length(3), // status
                    ],
                )
                .split(size);

                let header_area = vertical[0];
                let body_area = vertical[1];
                let input_area = vertical[2];
                let status_area = vertical[3];

                // Split body horizontally
                let body_split = Layout::new(
                    Direction::Horizontal,
                    [Constraint::Percentage(30), Constraint::Percentage(70)],
                )
                .split(body_area);

                let agents_area = body_split[0];
                let output_area = body_split[1];

                // Draw all panels
                ui::draw_header(f, header_area);
                ui::draw_agents(f, agents_area, &render_state, render_state.agent_scroll);
                ui::draw_output(f, output_area, &render_state, render_state.scroll_offset);
                ui::draw_input(f, input_area, &input_buffer, input_buffer.len());
                ui::draw_status(
                    f,
                    status_area,
                    render_state.connected,
                    render_state.agent_count(),
                    render_state.active_count(),
                );
            })
            .unwrap();

            needs_redraw = false;
        }

        // Poll for input events with timeout
        let timeout = std::time::Duration::from_millis(50);

        if let Ok(true) = crossterm::event::poll(timeout) {
            match event::read() {
                Ok(crossterm::event::Event::Key(key)) if key.kind == KeyEventKind::Press => {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            should_quit = true;
                        }

                        KeyCode::Char('c')
                            if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
                        {
                            should_quit = true;
                        }

                        KeyCode::Tab => {
                            focused_panel = (focused_panel + 1) % 3;
                            needs_redraw = true;
                        }

                        KeyCode::Up => {
                            if focused_panel == 1 {
                                render_state.agent_scroll = render_state.agent_scroll.saturating_sub(1);
                                needs_redraw = true;
                            } else if focused_panel == 2 {
                                render_state.scroll_offset = render_state.scroll_offset.saturating_sub(3);
                                needs_redraw = true;
                            }
                        }

                        KeyCode::Down => {
                            if focused_panel == 1 {
                                let max_scroll = render_state.agents.len().saturating_sub(1);
                                render_state.agent_scroll = (render_state.agent_scroll + 1).min(max_scroll);
                                needs_redraw = true;
                            } else if focused_panel == 2 {
                                render_state.scroll_offset += 3;
                                needs_redraw = true;
                            }
                        }

                        KeyCode::Char('l')
                            if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
                        {
                            if let Ok(mut s) = state.write() {
                                s.logs.clear();
                                render_state.logs.clear();
                                s.add_log(state::LogEntry::info("system", "Logs cleared"));
                                render_state.logs = s.logs.clone();
                            }
                            needs_redraw = true;
                        }

                        KeyCode::Char('r')
                            if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
                        {
                            if let Ok(mut s) = state.write() {
                                s.connected = false;
                                render_state.connected = false;
                            }
                            let st = state.clone();
                            tokio::spawn(async move {
                                match swarm::fetch_swarm_status().await {
                                    Ok(ok) => {
                                        if let Ok(mut s) = st.write() {
                                            s.connected = ok;
                                        }
                                    }
                                    Err(_) => {}
                                }
                            });
                            needs_redraw = true;
                        }

                        KeyCode::Enter => {
                            let goal = input_buffer.trim().to_string();
                            if !goal.is_empty() {
                                if let Ok(mut s) = state.write() {
                                    s.input_buffer.clear();
                                }
                                input_buffer.clear();

                                if let Ok(mut s) = state.write() {
                                    s.submit_goal(goal.clone());
                                    render_state = s.clone();
                                }
                                needs_redraw = true;

                                let st = state.clone();
                                let goal_clone = goal.clone();
                                tokio::spawn(async move {
                                    // Release lock before async operations
                                    drop(st.write().unwrap());
                                    // Simulate swarm work with temporary state
                                    let mut fake_state = AppState::new();
                                    run_swarm_goal(&mut fake_state, &goal_clone).await;
                                    // Merge results back
                                    if let Ok(mut guard) = st.try_write() {
                                        guard.logs = fake_state.logs.clone();
                                        guard.agents = fake_state.agents.clone();
                                    }
                                });
                            }
                        }

                        KeyCode::Backspace => {
                            input_buffer.pop();
                            needs_redraw = true;
                        }

                        KeyCode::Char(c) => {
                            input_buffer.push(c);
                            needs_redraw = true;
                        }

                        _ => {}
                    }
                }

                Ok(crossterm::event::Event::Resize(_, _)) => {
                    needs_redraw = true;
                }

                _ => {}
            }
        }

        // Sync shared state → render_state every frame
        if let Ok(s) = state.try_read() {
            if s.logs.len() != render_state.logs.len() {
                render_state.logs = s.logs.clone();
                render_state.agents = s.agents.clone();
                render_state.connected = s.connected;
                needs_redraw = true;
            }
        }

        if should_quit {
            break;
        }
    }

    // Cleanup
    disable_raw_mode().ok();
    io::stdout().execute(LeaveAlternateScreen).ok();
    io::stdout().execute(DisableBracketedPaste).ok();
    terminal.backend_mut().execute(cursor::Show).ok();

    println!("\n🐝 Goodbye from Gestalt Swarm TUI!\n");
}

fn main() -> anyhow::Result<()> {
    // Setup logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Starting Gestalt Swarm TUI v0.1.0");

    // Initialize shared state
    let state = new_shared_state();

    // Channel for input events
    let (_input_tx, input_rx) = mpsc::channel::<()>(100);

    // Run UI
    ui_loop(state, input_rx);

    Ok(())
}
