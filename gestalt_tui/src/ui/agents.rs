//! Agent list panel

use crate::state::{AgentStatus, AppState};
use ratatui::style::Stylize;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

fn status_color(status: AgentStatus) -> Color {
    match status {
        AgentStatus::Active => Color::Green,
        AgentStatus::Idle => Color::DarkGray,
        AgentStatus::Processing => Color::Yellow,
        AgentStatus::Error => Color::Red,
    }
}

/// Draw the agents panel (left side)
pub fn draw_agents(f: &mut Frame, area: Rect, state: &AppState, scroll: usize) {
    // Title
    let title = Line::from(vec![
        Span::styled("Agents", Style::new().bold().white()),
        Span::raw(" "),
        Span::styled(
            format!("({})", state.agent_count()),
            Style::new().dark_gray(),
        ),
    ]);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::new().dark_gray());

    let inner = block.inner(area);

    // Calculate visible items
    let item_height = 4; // lines per agent
    let visible_count = inner.height as usize / item_height;
    let total_height = state.agents.len() * item_height;

    // Build lines for all agents
    let mut lines: Vec<Line<'static>> = Vec::new();

    for (i, agent) in state.agents.iter().enumerate() {
        let is_selected = i == scroll;
        let sc = status_color(agent.status);
        let sym = agent.status.symbol();

        // Agent name
        let name = if is_selected {
            Line::from(vec![
                Span::raw("  "),
                Span::styled(sym, Style::new().fg(sc).bold()),
                Span::raw(" "),
                Span::styled(agent.name.clone(), Style::new().white().bold()),
            ])
        } else {
            Line::from(vec![
                Span::raw("  "),
                Span::styled(sym, Style::new().fg(sc)),
                Span::raw(" "),
                Span::styled(agent.name.clone(), Style::new().white()),
            ])
        };
        lines.push(name);

        // Status label
        let status_lbl = Line::from(vec![
            Span::raw("      "),
            Span::styled(
                agent.status.label().to_string(),
                Style::new().fg(sc).italic(),
            ),
        ]);
        lines.push(status_lbl);

        // Task line (owned string, no borrow issues)
        let task_str: String = if let Some(task) = &agent.current_task {
            let max = (inner.width as usize).saturating_sub(12);
            if task.len() > max && max > 4 {
                format!("{}...", &task[..max - 3])
            } else if task.len() > max {
                String::new()
            } else {
                task.clone()
            }
        } else {
            String::new()
        };
        let task_line = Line::from(vec![
            Span::raw("      "),
            Span::styled(task_str, Style::new().dark_gray()),
        ]);
        lines.push(task_line);

        // Spacer
        lines.push(Line::from(vec![Span::raw("")]));
    }

    let content = Paragraph::new(lines)
        .block(block)
        .style(Style::new().on_black());

    f.render_widget(content, area);

    // Scrollbar
    if total_height > visible_count {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::new().fg(Color::DarkGray));

        let mut scrollbar_state =
            ScrollbarState::new(total_height).position(scroll * item_height);

        f.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}
