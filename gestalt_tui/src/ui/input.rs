//! Goal input bar (bottom)

use ratatui::style::Stylize;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// Draw the goal input bar
pub fn draw_input(f: &mut Frame, area: Rect, input: &str, _cursor_pos: usize) {
    let prompt = Span::styled("> ", Style::new().cyan().bold());
    let text = Span::raw(input);
    let cursor = Span::styled(" ", Style::new().black().on_white());

    let line = Line::from(vec![prompt, text, cursor]);

    let paragraph = Paragraph::new(line)
        .style(Style::new().on_black().black())
        .block(
            ratatui::widgets::Block::default()
                .title(" Goal ")
                .borders(ratatui::widgets::Borders::ALL)
                .border_style(Style::new().cyan()),
        );

    f.render_widget(paragraph, area);
}
