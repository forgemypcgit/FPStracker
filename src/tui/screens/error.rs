//! Error modal drawing with rounded border.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};

use crate::tui::state::{App, ModalKind};
use crate::tui::theme::Theme;

pub(crate) fn draw_error_modal(area: Rect, f: &mut ratatui::Frame, app: &App, theme: Theme) {
    let modal = match &app.error_modal {
        Some(m) => m,
        None => return,
    };

    let (icon, color, footer) = match modal.kind {
        ModalKind::Error => ("✕", theme.critical, "[Enter to continue]"),
        ModalKind::Info => ("i", theme.oracle, "[Enter to continue]"),
        ModalKind::Confirm => ("?", theme.caution, "[Enter to confirm]  [Esc to cancel]"),
    };

    let popup_area = centered_rect(80, 60, area);
    f.render_widget(Clear, popup_area);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(format!("  {icon}  "), Style::default().fg(color)),
        Span::styled(
            &modal.title,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(""));

    for line in modal.message.lines() {
        lines.push(Line::from(Span::styled(
            format!("  {line}"),
            Style::default().fg(theme.text),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("  {footer}"),
        Style::default().fg(theme.muted),
    )));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color));

    let para = Paragraph::new(Text::from(lines))
        .block(block)
        .wrap(Wrap { trim: true });
    f.render_widget(para, popup_area);
}

pub(crate) fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
