//! Context-sensitive keybind footer bar.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::tui::theme::Theme;

pub(crate) fn draw_footer(
    area: Rect,
    f: &mut ratatui::Frame,
    theme: &Theme,
    hints: &[(&str, &str)],
) {
    let mut spans = Vec::new();
    for (i, (key, action)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" │ ", Style::default().fg(theme.muted)));
        }
        spans.push(Span::styled(*key, Style::default().fg(theme.oracle)));
        spans.push(Span::styled(
            format!(" {action}"),
            Style::default().fg(theme.muted),
        ));
    }
    let footer = Paragraph::new(Line::from(spans));
    f.render_widget(footer, area);
}
