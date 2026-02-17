//! Visual step stepper for contribute flow.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::tui::theme::Theme;

const STEP_LABELS: [&str; 6] = [
    "Consent", "Hardware", "Baseline", "Game", "Results", "Review",
];

pub(crate) fn draw_progress(
    area: Rect,
    f: &mut ratatui::Frame,
    theme: &Theme,
    current_step: usize,
) {
    let mut top_spans = Vec::new();
    let mut label_spans = Vec::new();

    for (i, label) in STEP_LABELS.iter().enumerate() {
        let step_num = i + 1;
        let is_completed = step_num < current_step;
        let is_current = step_num == current_step;

        let (num_style, connector_char) = if is_completed {
            (
                Style::default()
                    .fg(theme.optimal)
                    .add_modifier(Modifier::BOLD),
                '━',
            )
        } else if is_current {
            (
                Style::default()
                    .fg(theme.oracle)
                    .add_modifier(Modifier::BOLD),
                '─',
            )
        } else {
            (Style::default().fg(theme.muted), '─')
        };

        let label_style = if is_current {
            Style::default()
                .fg(theme.oracle)
                .add_modifier(Modifier::BOLD)
        } else if is_completed {
            Style::default().fg(theme.optimal)
        } else {
            Style::default().fg(theme.muted)
        };

        // Step number bubble
        if is_current {
            top_spans.push(Span::styled(format!("[ {step_num} ]"), num_style));
        } else {
            top_spans.push(Span::styled(format!("[{step_num}]"), num_style));
        }

        // Connector between steps
        if i < STEP_LABELS.len() - 1 {
            top_spans.push(Span::styled(
                format!("{0}{0}{0}", connector_char),
                if is_completed {
                    Style::default().fg(theme.optimal)
                } else {
                    Style::default().fg(theme.muted)
                },
            ));
        }

        // Build label line with padding to align under numbers
        let pad = if is_current {
            // "[ N ]" = 5 chars
            let total: usize = 5;
            let lpad = total.saturating_sub(label.len()) / 2;
            " ".repeat(lpad)
        } else {
            // "[N]" = 3 chars
            let total: usize = 3;
            let lpad = total.saturating_sub(label.len()) / 2;
            " ".repeat(lpad)
        };
        label_spans.push(Span::styled(format!("{pad}{label}"), label_style));
        if i < STEP_LABELS.len() - 1 {
            // Spacer for connector
            label_spans.push(Span::styled("   ", Style::default().fg(theme.muted)));
        }
    }

    let stepper = Paragraph::new(vec![Line::from(top_spans), Line::from(label_spans)]);
    f.render_widget(stepper, area);
}
