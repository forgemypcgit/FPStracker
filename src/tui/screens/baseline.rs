//! Baseline screen drawing with gauge bars.

use std::time::Duration;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Wrap};

use crate::tui::state::App;
use crate::tui::theme::Theme;
use crate::tui::widgets::card::CardWidget;
use crate::tui::widgets::gauge::draw_gauge;

pub(crate) fn draw_contribute_baseline(
    area: Rect,
    f: &mut ratatui::Frame,
    app: &App,
    theme: Theme,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // description
            Constraint::Min(0),    // content
        ])
        .split(area);

    let desc = Paragraph::new(Text::from(vec![
        Line::from(Span::styled(
            "Optional: run a synthetic baseline (local only).",
            Style::default().fg(theme.text),
        )),
        Line::from(Span::styled(
            "This does not upload by itself. Scores are attached only if you submit.",
            Style::default().fg(theme.muted),
        )),
    ]))
    .wrap(Wrap { trim: true });
    f.render_widget(desc, chunks[0]);

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    if let Some(b) = app.contribute.baseline.as_ref() {
        // Show score gauges
        let gauge_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(0),
            ])
            .split(chunks[1]);

        let scores = [
            ("CPU", b.cpu_score, b.cpu_score_source.as_deref()),
            ("GPU", b.gpu_score, b.gpu_score_source.as_deref()),
            ("RAM", b.ram_score, b.ram_score_source.as_deref()),
            ("Disk", b.disk_score, b.disk_score_source.as_deref()),
        ];

        for (i, (label, value, source)) in scores.iter().enumerate() {
            if let Some(v) = value {
                let label = format_score_label(label, *source);
                draw_gauge(
                    gauge_area[i],
                    f,
                    &theme,
                    &label,
                    *v as f64,
                    gauge_max_for_source(*source),
                    &format!("{}", v),
                );
            } else {
                let line = Paragraph::new(Line::from(vec![
                    Span::styled(format!("{:<12}", label), Style::default().fg(theme.text)),
                    Span::styled("—", Style::default().fg(theme.muted)),
                ]));
                f.render_widget(line, gauge_area[i]);
            }
        }
        return;
    }

    #[allow(unreachable_code)]
    {
        let card = CardWidget::new("Baseline")
            .line(Line::from(""))
            .line(Line::from(Span::styled(
                "  No baseline recorded yet.",
                Style::default().fg(theme.muted),
            )))
            .line(Line::from(Span::styled(
                "  Press B to run the standard baseline, or Enter/S to skip.",
                Style::default().fg(theme.text_dim),
            )))
            .line(Line::from(Span::styled(
                "  Linux: run `fps-tracker doctor --fix` to install optional tools (or press I to show the command).",
                Style::default().fg(theme.text_dim),
            )));
        card.render(chunks[1], f, &theme);
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub(crate) fn draw_synthetic_running(area: Rect, f: &mut ratatui::Frame, app: &App, theme: Theme) {
    let elapsed = app
        .synthetic
        .as_ref()
        .map(|s| s.started_at.elapsed())
        .unwrap_or_else(|| Duration::from_secs(0));
    let spinner = app.animation.spinner_char();
    let progress = app.synthetic_progress.as_ref();
    let total_steps = progress.map(|p| p.total_steps.max(1)).unwrap_or(4);
    let completed_steps = progress
        .map(|p| p.completed_steps.min(total_steps))
        .unwrap_or(0);
    let status = progress
        .map(|p| p.status.as_str())
        .unwrap_or("Preparing synthetic benchmarks");
    let percent = ((completed_steps as f64 / total_steps as f64) * 100.0).round() as u64;
    let width = 22usize;
    let filled = ((percent as f64 / 100.0) * width as f64).round() as usize;
    let bar = format!(
        "{}{}",
        "#".repeat(filled.min(width)),
        "-".repeat(width.saturating_sub(filled.min(width)))
    );

    let card = CardWidget::new("Synthetic Baseline — Running")
        .border_color(theme.oracle)
        .line(Line::from(""))
        .line(Line::from(vec![
            Span::styled(format!("  {spinner}  "), Style::default().fg(theme.oracle)),
            Span::styled(
                format!("Elapsed: {:.1}s", elapsed.as_secs_f64()),
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
        ]))
        .line(Line::from(vec![
            Span::styled("  Stage: ", Style::default().fg(theme.text_dim)),
            Span::styled(status, Style::default().fg(theme.text)),
        ]))
        .line(Line::from(vec![
            Span::styled("  Progress: ", Style::default().fg(theme.text_dim)),
            Span::styled(
                format!("[{bar}] {percent:>3}% ({completed_steps}/{total_steps})"),
                Style::default().fg(theme.oracle),
            ),
        ]))
        .line(Line::from(""))
        .line(Line::from(Span::styled(
            "  This runs locally and does not upload anything.",
            Style::default().fg(theme.muted),
        )))
        .line(Line::from(Span::styled(
            "  Press Esc to cancel.",
            Style::default().fg(theme.text_dim),
        )));
    card.render(area, f, &theme);
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub(crate) fn draw_synthetic_result(area: Rect, f: &mut ratatui::Frame, app: &App, theme: Theme) {
    if let Some(results) = app.synthetic_result.as_ref() {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(0),
            ])
            .split(area);

        let header = Paragraph::new(Line::from(Span::styled(
            "Synthetic baseline complete",
            Style::default()
                .fg(theme.optimal)
                .add_modifier(Modifier::BOLD),
        )));
        f.render_widget(header, chunks[0]);

        let scores = [
            (
                "CPU",
                results.cpu_score,
                results.cpu_score_source.as_deref(),
            ),
            (
                "GPU",
                results.gpu_score,
                results.gpu_score_source.as_deref(),
            ),
            (
                "RAM",
                results.ram_score,
                results.ram_score_source.as_deref(),
            ),
            (
                "Disk",
                results.disk_score,
                results.disk_score_source.as_deref(),
            ),
        ];

        for (i, (label, value, source)) in scores.iter().enumerate() {
            if let Some(v) = value {
                let label = format_score_label(label, *source);
                draw_gauge(
                    chunks[i + 1],
                    f,
                    &theme,
                    &label,
                    *v as f64,
                    gauge_max_for_source(*source),
                    &format!("{}", v),
                );
            } else {
                let line = Paragraph::new(Line::from(vec![
                    Span::styled(format!("{:<12}", label), Style::default().fg(theme.text)),
                    Span::styled("—", Style::default().fg(theme.muted)),
                ]));
                f.render_widget(line, chunks[i + 1]);
            }
        }

        let hint = Paragraph::new(Line::from(Span::styled(
            "  Press Enter to continue.",
            Style::default().fg(theme.text_dim),
        )));
        f.render_widget(hint, chunks[5]);
    } else {
        let error_msg = app
            .synthetic_error
            .clone()
            .unwrap_or_else(|| "Unknown error.".to_string());

        let card = CardWidget::new("Synthetic Baseline — Error")
            .border_color(theme.critical)
            .line(Line::from(""))
            .line(Line::from(Span::styled(
                format!("  {error_msg}"),
                Style::default().fg(theme.critical),
            )))
            .line(Line::from(""))
            .line(Line::from(Span::styled(
                "  Press Enter to continue without scores.",
                Style::default().fg(theme.text_dim),
            )));
        card.render(area, f, &theme);
    }
}

fn format_score_label(label: &str, source: Option<&str>) -> String {
    let Some(source) = source else {
        return format!("{label} score");
    };
    let source = source.trim();
    if source.is_empty() {
        return format!("{label} score");
    }
    format!("{label} ({source})")
}

fn gauge_max_for_source(source: Option<&str>) -> f64 {
    match source.unwrap_or_default().trim() {
        "winsat" => 1000.0,
        "diskspd_read_mib_s" => 8000.0,
        "7z_mips" => 200_000.0,
        _ => 10_000.0,
    }
}
