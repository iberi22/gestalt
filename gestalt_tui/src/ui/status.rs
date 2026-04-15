//! Status bar (bottom)

use ratatui::style::Stylize;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// Draw the status bar
pub fn draw_status(f: &mut Frame, area: Rect, connected: bool, agent_count: usize, active: usize) {
    let conn = if connected {
        (Color::Green, "connected")
    } else {
        (Color::Red, "disconnected")
    };

    let status_line = Line::from(vec![
        Span::raw("  "),
        Span::styled("●", Style::new().fg(conn.0)),
        Span::raw(" "),
        Span::styled(conn.1, Style::new().fg(conn.0)),
        Span::raw("     "),
        Span::styled("Agents:", Style::new().dark_gray()),
        Span::raw(" "),
        Span::styled(agent_count.to_string(), Style::new().white()),
        Span::raw(" "),
        Span::styled("Active:", Style::new().dark_gray()),
        Span::raw(" "),
        Span::styled(active.to_string(), Style::new().cyan()),
        Span::raw("          "),
        Span::styled("q:quit  Tab:focus  ↑↓:scroll  Ctrl+L:clear", Style::new().dark_gray()),
    ]);

    let paragraph = Paragraph::new(status_line)
        .style(Style::new().on_dark_gray().black())
        .alignment(ratatui::layout::Alignment::Left);

    f.render_widget(paragraph, area);
}
