//! Results entry screen with FPS quality indicator.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};

use crate::tui::state::{App, InputMode};
use crate::tui::theme::Theme;

pub(crate) fn draw_contribute_results(area: Rect, f: &mut ratatui::Frame, app: &App, theme: Theme) {
    let s = &app.contribute.results;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // FPS quality hint
            Constraint::Min(0),    // form
        ])
        .split(area);

    // FPS quality indicator (if avg_fps is filled)
    if let Ok(fps) = s.avg_fps.trim().parse::<f64>() {
        if fps > 0.0 {
            let color = theme.fps_color(fps);
            let label = Theme::fps_label(fps);
            let bar_width = 20usize;
            let ratio = (fps / 240.0).clamp(0.0, 1.0);
            let filled = (bar_width as f64 * ratio) as usize;
            let empty = bar_width.saturating_sub(filled);

            let quality = Paragraph::new(Line::from(vec![
                Span::styled("  Avg FPS  ", Style::default().fg(theme.text_dim)),
                Span::styled("█".repeat(filled), Style::default().fg(color)),
                Span::styled("░".repeat(empty), Style::default().fg(theme.muted)),
                Span::styled(
                    format!("  {fps:.0} FPS — {label}"),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
            ]));
            f.render_widget(quality, chunks[0]);
        }
    }

    // Form fields
    let mut lines: Vec<Line> = Vec::new();

    let rows: Vec<(usize, &str, String, bool)> = vec![
        (0, "Resolution", s.resolution.clone(), false),
        (1, "Preset", s.preset.clone(), false),
        (2, "Avg FPS", s.avg_fps.clone(), false),
        (3, "1% low (opt)", s.fps_1_low.clone(), false),
        (4, "0.1% low (opt)", s.fps_01_low.clone(), false),
        (
            5,
            "Ray tracing",
            if s.ray_tracing { "ON" } else { "OFF" }.to_string(),
            true,
        ),
        (
            6,
            "Capture method",
            s.capture_method.label().to_string(),
            true,
        ),
        (
            7,
            "Anti-cheat ack",
            if s.anti_cheat_ack { "YES" } else { "NO" }.to_string(),
            true,
        ),
        (8, "Upscaling (opt)", s.upscaling.clone(), false),
    ];

    for (idx, label, value, is_toggle) in rows {
        let selected = idx == s.cursor;
        let editing = selected && s.mode == InputMode::Edit && !is_toggle;

        let selector_style = if selected {
            Style::default().fg(theme.oracle)
        } else {
            Style::default().fg(theme.muted)
        };
        let label_style = if selected {
            Style::default()
                .fg(theme.oracle)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text)
        };

        let display = if value.trim().is_empty() {
            "—".to_string()
        } else {
            value.clone()
        };

        let value_style = if editing {
            Style::default()
                .fg(theme.oracle)
                .add_modifier(Modifier::UNDERLINED)
        } else if is_toggle && selected {
            Style::default()
                .fg(theme.oracle)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text)
        };

        let mut spans = vec![
            Span::styled(if selected { " › " } else { "   " }, selector_style),
            Span::styled(format!("{:<18}", label), label_style),
            Span::styled(display, value_style),
        ];

        if editing {
            spans.push(Span::styled(" [EDIT]", Style::default().fg(theme.oracle)));
        }
        if is_toggle && selected {
            spans.push(Span::styled(
                if idx == 6 { " ◄ ►" } else { " [Space]" },
                Style::default().fg(theme.text_dim),
            ));
        }

        lines.push(Line::from(spans));
    }

    let body = Paragraph::new(Text::from(lines))
        .block(
            Block::default()
                .title(Span::styled(
                    " Results ",
                    Style::default()
                        .fg(theme.oracle)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(theme.border)),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(body, chunks[1]);
}
