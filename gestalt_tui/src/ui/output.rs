//! Output / log panel

use crate::state::{AppState, LogLevel};
use ratatui::{
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

fn log_level_color(level: LogLevel) -> Color {
    match level {
        LogLevel::Info => Color::White,
        LogLevel::Warn => Color::Yellow,
        LogLevel::Error => Color::Red,
        LogLevel::Success => Color::Green,
        LogLevel::Debug => Color::DarkGray,
    }
}

/// Draw the output log panel (right side)
pub fn draw_output(f: &mut Frame, area: Rect, state: &AppState, scroll_offset: usize) {
    let title = Line::from(vec![
        Span::styled("Output", Style::new().bold().white()),
        Span::raw(" "),
        Span::styled(
            format!("({})", state.logs.len()),
            Style::new().dark_gray(),
        ),
    ]);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::new().dark_gray());

    let inner = block.inner(area);

    // Build owned log lines
    let mut lines: Vec<Line<'static>> = Vec::new();

    for entry in state.logs.iter().skip(scroll_offset) {
        if lines.len() >= inner.height as usize {
            break;
        }

        let ts_str = entry.timestamp.format("%H:%M:%S").to_string();
        let src_str = entry.source.clone();
        let msg_str = entry.message.clone();
        let lc = log_level_color(entry.level);

        let symbol = match entry.level {
            LogLevel::Info => "",
            LogLevel::Warn => "⚠ ",
            LogLevel::Error => "✗ ",
            LogLevel::Success => "✓ ",
            LogLevel::Debug => "  ",
        };

        lines.push(Line::from(vec![
            Span::styled(ts_str, Style::new().dark_gray()),
            Span::raw(" "),
            Span::styled(src_str, Style::new().cyan().bold()),
            Span::raw(": "),
            Span::styled(symbol, Style::new().fg(lc)),
            Span::raw(msg_str),
        ]));
    }

    let content = Paragraph::new(lines)
        .block(block)
        .style(Style::new().on_black());

    f.render_widget(content, area);

    // Scrollbar if needed
    if state.logs.len() > inner.height as usize {
        let scrollbar =
            Scrollbar::new(ScrollbarOrientation::VerticalRight).style(Style::new().fg(Color::DarkGray));

        let total = state.logs.len().max(1);
        let visible = inner.height as usize;
        let position = scroll_offset.min(total.saturating_sub(visible));

        let mut scrollbar_state = ScrollbarState::new(total).position(position);

        f.render_stateful_widget(scrollbar, inner, &mut scrollbar_state);
    }
}
