//! Horizontal score/FPS gauge bar.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::tui::theme::Theme;

pub(crate) fn draw_gauge(
    area: Rect,
    f: &mut ratatui::Frame,
    theme: &Theme,
    label: &str,
    value: f64,
    max: f64,
    suffix: &str,
) {
    let label_width = 12;
    let suffix_str = format!(" {:>8}", suffix);
    let bar_width = (area.width as usize)
        .saturating_sub(label_width)
        .saturating_sub(suffix_str.len());

    let ratio = (value / max).clamp(0.0, 1.0);
    let filled = (bar_width as f64 * ratio) as usize;
    let empty = bar_width.saturating_sub(filled);

    let filled_str: String = "█".repeat(filled);
    let empty_str: String = "░".repeat(empty);

    let line = Line::from(vec![
        Span::styled(
            format!("{:<width$}", label, width = label_width),
            Style::default().fg(theme.text),
        ),
        Span::styled(filled_str, Style::default().fg(theme.oracle)),
        Span::styled(empty_str, Style::default().fg(theme.muted)),
        Span::styled(suffix_str, Style::default().fg(theme.text)),
    ]);

    f.render_widget(Paragraph::new(line), area);
}
