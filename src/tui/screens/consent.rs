//! Consent screen drawing with informational cards and updated legal text.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};

use crate::tui::state::App;
use crate::tui::theme::Theme;

#[allow(clippy::vec_init_then_push)]
pub(crate) fn draw_contribute_consent(area: Rect, f: &mut ratatui::Frame, app: &App, theme: Theme) {
    // Split into info cards area + checkboxes + progress indicator
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // Info cards + checkboxes (scrollable area)
            Constraint::Length(1), // Progress indicator
        ])
        .split(area);

    let mut lines: Vec<Line> = Vec::new();

    // Info card 1: What we collect
    lines.push(Line::from(Span::styled(
        "╭─ What we collect ────────────────────────────────────────╮",
        Style::default().fg(theme.border),
    )));
    lines.push(Line::from(vec![
        Span::styled("│ ", Style::default().fg(theme.border)),
        Span::styled(
            "GPU/CPU/RAM specs, game settings (resolution, preset),",
            Style::default().fg(theme.text),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("│ ", Style::default().fg(theme.border)),
        Span::styled(
            "FPS metrics (avg, 1% low, 0.1% low), capture method",
            Style::default().fg(theme.text),
        ),
    ]));
    lines.push(Line::from(Span::styled(
        "╰──────────────────────────────────────────────────────────╯",
        Style::default().fg(theme.border),
    )));

    lines.push(Line::from(""));

    // Info card 2: How data is used
    lines.push(Line::from(Span::styled(
        "╭─ How data is used ───────────────────────────────────────╮",
        Style::default().fg(theme.border),
    )));
    lines.push(Line::from(vec![
        Span::styled("│ ", Style::default().fg(theme.border)),
        Span::styled(
            "FPS prediction, build recommendations, aggregate stats.",
            Style::default().fg(theme.text),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("│ ", Style::default().fg(theme.border)),
        Span::styled(
            "Data may be used commercially by the project owner.",
            Style::default()
                .fg(theme.caution)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(Span::styled(
        "╰──────────────────────────────────────────────────────────╯",
        Style::default().fg(theme.border),
    )));

    lines.push(Line::from(""));

    // Info card 3: What we skip
    lines.push(Line::from(Span::styled(
        "╭─ What we skip ───────────────────────────────────────────╮",
        Style::default().fg(theme.border),
    )));
    lines.push(Line::from(vec![
        Span::styled("│ ", Style::default().fg(theme.border)),
        Span::styled(
            "No serial numbers, UUIDs, file lists, or personal data",
            Style::default().fg(theme.text),
        ),
    ]));
    lines.push(Line::from(Span::styled(
        "╰──────────────────────────────────────────────────────────╯",
        Style::default().fg(theme.border),
    )));

    lines.push(Line::from(""));

    // Info card 4: Retention & rights
    lines.push(Line::from(Span::styled(
        "╭─ Retention & rights ─────────────────────────────────────╮",
        Style::default().fg(theme.border),
    )));
    lines.push(Line::from(vec![
        Span::styled("│ ", Style::default().fg(theme.border)),
        Span::styled(
            "Up to 10 years. Anonymous — cannot delete specific records.",
            Style::default().fg(theme.text),
        ),
    ]));
    lines.push(Line::from(Span::styled(
        "╰──────────────────────────────────────────────────────────╯",
        Style::default().fg(theme.border),
    )));

    lines.push(Line::from(""));

    // Checkbox section
    let bullets = [
        (
            app.contribute.consent.tos,
            "I agree to the Terms of Service",
        ),
        (
            app.contribute.consent.public_use,
            "I consent to anonymized data being used publicly, including for commercial statistics and FPS prediction",
        ),
        (
            app.contribute.consent.retention,
            "I understand retention may be up to 10 years",
        ),
    ];

    for (idx, (checked, label)) in bullets.into_iter().enumerate() {
        let selected = idx == app.contribute.consent.cursor;
        let check = if checked { "◉" } else { "○" };
        let check_style = if checked {
            Style::default().fg(theme.optimal)
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

        lines.push(Line::from(vec![
            Span::styled(if selected { " › " } else { "   " }, label_style),
            Span::styled(format!("{check} "), check_style),
            Span::styled(label, label_style),
        ]));
    }

    // Render scrollable content
    let scroll_offset = app.contribute.consent.scroll_offset;
    let content = Paragraph::new(Text::from(lines))
        .block(
            Block::default()
                .title(Span::styled(
                    " Consent ",
                    Style::default()
                        .fg(theme.oracle)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(theme.border)),
        )
        .wrap(Wrap { trim: true })
        .scroll((scroll_offset, 0));
    f.render_widget(content, chunks[0]);

    // Progress indicator
    let checked_count = [
        app.contribute.consent.tos,
        app.contribute.consent.public_use,
        app.contribute.consent.retention,
    ]
    .iter()
    .filter(|&&v| v)
    .count();

    let progress_style = if checked_count == 3 {
        Style::default()
            .fg(theme.optimal)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text_dim)
    };
    let progress = Paragraph::new(Line::from(Span::styled(
        format!("  {checked_count}/3 confirmed"),
        progress_style,
    )));
    f.render_widget(progress, chunks[1]);
}
