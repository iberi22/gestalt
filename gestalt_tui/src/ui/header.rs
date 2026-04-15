//! Header widget — logo + swarm status

use ratatui::style::Stylize;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// Draw the header bar
pub fn draw_header(f: &mut Frame, area: Rect) {
    let logo = Line::from(vec![
        Span::raw("🐝 "),
        Span::styled("Gestalt", Style::new().bold().cyan()),
        Span::raw(" Swarm "),
        Span::styled("TUI", Style::new().bold().white()),
    ]);

    let status = Line::from(vec![
        Span::raw(" ● "),
        Span::styled("connected", Style::new().green()),
    ]);

    let version = Line::from(vec![
        Span::styled("v0.1.0", Style::new().dark_gray()),
    ]);

    let paragraph = Paragraph::new(vec![logo, status, version])
        .style(Style::new().black().on_dark_gray())
        .alignment(ratatui::layout::Alignment::Left);

    f.render_widget(paragraph, area);
}
