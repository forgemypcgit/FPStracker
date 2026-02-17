//! Top header bar with title and context.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::tui::theme::Theme;

pub(crate) fn draw_header(
    area: Rect,
    f: &mut ratatui::Frame,
    theme: &Theme,
    context: Option<&str>,
) {
    let mut spans = vec![Span::styled(
        "FPS TRACKER",
        Style::default()
            .fg(theme.oracle)
            .add_modifier(Modifier::BOLD),
    )];

    if let Some(ctx) = context {
        spans.push(Span::styled(
            format!("  //  {ctx}"),
            Style::default().fg(theme.text_dim),
        ));
    }

    let header_line = Line::from(spans);
    let rule = Line::from(Span::styled(
        "─".repeat(area.width as usize),
        Style::default().fg(theme.border),
    ));

    f.render_widget(Paragraph::new(vec![header_line, rule]), area);
}
