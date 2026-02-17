//! Feedback screen drawing.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Wrap};

use crate::tui::animation::SUCCESS_CHECKMARK;
use crate::tui::state::{App, FeedbackStep};
use crate::tui::theme::Theme;
use crate::tui::widgets::footer::draw_footer;

pub(crate) fn draw_feedback(
    area: Rect,
    f: &mut ratatui::Frame,
    app: &App,
    theme: Theme,
    step: FeedbackStep,
) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let header = Paragraph::new(Text::from(vec![
        Line::from(Span::styled(
            "Feedback",
            Style::default()
                .fg(theme.oracle)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            app.schema.intro,
            Style::default().fg(theme.text),
        )),
        Line::from(Span::styled(
            app.schema.privacy_note,
            Style::default().fg(theme.muted),
        )),
    ]))
    .wrap(Wrap { trim: true });
    f.render_widget(header, layout[0]);

    match step {
        FeedbackStep::Category => draw_feedback_category(layout[1], f, app, &theme),
        FeedbackStep::Issue => draw_feedback_issue(layout[1], f, app, &theme),
        FeedbackStep::Message | FeedbackStep::Submitting => {
            draw_feedback_message(layout[1], f, app, &theme, step == FeedbackStep::Submitting)
        }
    }

    let hints: &[(&str, &str)] = match step {
        FeedbackStep::Category => &[("↑/↓", "Category"), ("Enter", "Next"), ("Esc", "Back")],
        FeedbackStep::Issue => &[("↑/↓", "Issue"), ("Enter", "Next"), ("Esc", "Back")],
        FeedbackStep::Message => &[("F5", "Submit"), ("Tab", "Diagnostics"), ("Esc", "Back")],
        FeedbackStep::Submitting => &[],
    };
    draw_footer(layout[2], f, &theme, hints);
}

fn draw_feedback_category(area: Rect, f: &mut ratatui::Frame, app: &App, theme: &Theme) {
    let items = app
        .schema
        .categories
        .iter()
        .enumerate()
        .map(|(idx, cat)| {
            let selected = idx == app.feedback.category_index;
            let style = if selected {
                Style::default()
                    .fg(theme.oracle)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text)
            };
            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(if selected { " › " } else { "   " }, style),
                    Span::styled(cat.label, style),
                ]),
                Line::from(vec![
                    Span::raw("     "),
                    Span::styled(cat.description, Style::default().fg(theme.muted)),
                ]),
            ])
        })
        .collect::<Vec<_>>();

    let list = List::new(items).block(
        Block::default()
            .title(Span::styled(
                " 1/3 Category ",
                Style::default().fg(theme.text_dim),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.border)),
    );
    f.render_widget(list, area);
}

fn draw_feedback_issue(area: Rect, f: &mut ratatui::Frame, app: &App, theme: &Theme) {
    let issues = &app.category().issues;
    let items = issues
        .iter()
        .enumerate()
        .map(|(idx, issue)| {
            let selected = idx == app.feedback.issue_index;
            let style = if selected {
                Style::default()
                    .fg(theme.oracle)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text)
            };
            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(if selected { " › " } else { "   " }, style),
                    Span::styled(issue.label, style),
                ]),
                Line::from(vec![
                    Span::raw("     "),
                    Span::styled(issue.hint, Style::default().fg(theme.muted)),
                ]),
            ])
        })
        .collect::<Vec<_>>();

    let list = List::new(items).block(
        Block::default()
            .title(Span::styled(
                " 2/3 Issue ",
                Style::default().fg(theme.text_dim),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.border)),
    );
    f.render_widget(list, area);
}

fn draw_feedback_message(
    area: Rect,
    f: &mut ratatui::Frame,
    app: &App,
    theme: &Theme,
    submitting: bool,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(6), Constraint::Length(3)])
        .split(area);

    let message_block = Block::default()
        .title(Span::styled(
            " 3/3 Message ",
            Style::default().fg(theme.text_dim),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border));

    let message = if app.feedback.message.is_empty() {
        Text::from(vec![
            Line::from(Span::styled(
                "Type what happened (include what you expected vs what happened).",
                Style::default().fg(theme.muted),
            )),
            Line::from(Span::styled(
                "Avoid personal info. We'll redact common identifiers, but it's best to omit them.",
                Style::default().fg(theme.muted),
            )),
        ])
    } else {
        Text::from(app.feedback.message.as_str())
    };

    let para = Paragraph::new(message)
        .block(message_block)
        .wrap(Wrap { trim: false });
    f.render_widget(para, chunks[0]);

    let diag_label = if app.feedback.include_diagnostics {
        Span::styled(
            "Diagnostics: ON",
            Style::default().fg(theme.good).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            "Diagnostics: OFF",
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        )
    };
    let hint = if submitting {
        Span::styled("Submitting...", Style::default().fg(theme.muted))
    } else {
        Span::styled(
            "Tab toggles diagnostics. F5 or Ctrl+S submits.",
            Style::default().fg(theme.muted),
        )
    };

    let footer = Paragraph::new(Line::from(vec![diag_label, Span::raw("   "), hint])).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.border)),
    );
    f.render_widget(footer, chunks[1]);
}

pub(crate) fn draw_feedback_result(area: Rect, f: &mut ratatui::Frame, app: &App, theme: Theme) {
    let state = match &app.feedback_result {
        Some(s) => s,
        None => return,
    };

    let title_lc = state.title.to_ascii_lowercase();
    let is_success = title_lc.contains("sent") || state.title.contains("Queued");

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));

    if is_success {
        for art_line in SUCCESS_CHECKMARK {
            lines.push(Line::from(Span::styled(
                format!("       {art_line}"),
                Style::default()
                    .fg(theme.optimal)
                    .add_modifier(Modifier::BOLD),
            )));
        }
        lines.push(Line::from(""));
    }

    lines.push(Line::from(Span::styled(
        format!("  {}", state.title),
        Style::default()
            .fg(if is_success {
                theme.optimal
            } else {
                theme.text
            })
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    for line in state.body.lines() {
        lines.push(Line::from(Span::styled(
            format!("  {line}"),
            Style::default().fg(theme.text),
        )));
    }

    let border_color = if is_success {
        theme.optimal
    } else {
        theme.border
    };
    let block = Block::default()
        .title(Span::styled(
            " Result ",
            Style::default()
                .fg(if is_success {
                    theme.optimal
                } else {
                    theme.oracle
                })
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));

    let para = Paragraph::new(Text::from(lines))
        .block(block)
        .wrap(Wrap { trim: true });
    f.render_widget(para, area);
}
