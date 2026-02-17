//! Review screen with collapsible sections + result screen.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};

use crate::tui::input::build_submission_preview;
use crate::tui::state::App;
use crate::tui::theme::Theme;
use crate::tui::widgets::banner::draw_wordmark;

pub(crate) fn draw_contribute_review(area: Rect, f: &mut ratatui::Frame, app: &App, theme: Theme) {
    let (submission, issues) = build_submission_preview(app);

    let mut lines: Vec<Line> = Vec::new();

    if let Some(issues) = issues {
        lines.push(Line::from(Span::styled(
            " Fix the following before submitting:",
            Style::default()
                .fg(theme.critical)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        for issue in issues {
            lines.push(Line::from(vec![
                Span::styled("  • ", Style::default().fg(theme.critical)),
                Span::styled(issue, Style::default().fg(theme.text)),
            ]));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Press Esc to go back and fix.",
            Style::default().fg(theme.muted),
        )));
    } else if let Some(s) = submission {
        let expanded = app.contribute.review_expanded;

        // Section 1: Hardware
        let hw_toggle = if expanded[0] { "▼" } else { "►" };
        lines.push(Line::from(vec![
            Span::styled(format!(" {hw_toggle} "), Style::default().fg(theme.oracle)),
            Span::styled(
                "[1] Hardware",
                Style::default()
                    .fg(theme.oracle)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        if expanded[0] {
            lines.push(Line::from(vec![
                Span::styled("     GPU: ", Style::default().fg(theme.text_dim)),
                Span::styled(
                    s.system_info.gpu.name.clone(),
                    Style::default().fg(theme.text),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("     CPU: ", Style::default().fg(theme.text_dim)),
                Span::styled(
                    s.system_info.cpu.name.clone(),
                    Style::default().fg(theme.text),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("     RAM: ", Style::default().fg(theme.text_dim)),
                Span::styled(
                    format!("{} MB", s.system_info.ram.usable_mb),
                    Style::default().fg(theme.text),
                ),
            ]));
        }
        lines.push(Line::from(""));

        // Section 2: Baseline
        let bl_toggle = if expanded[1] { "▼" } else { "►" };
        lines.push(Line::from(vec![
            Span::styled(format!(" {bl_toggle} "), Style::default().fg(theme.oracle)),
            Span::styled(
                "[2] Baseline",
                Style::default()
                    .fg(theme.oracle)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        if expanded[1] {
            let scores = [
                ("CPU", s.synthetic_cpu_score),
                ("GPU", s.synthetic_gpu_score),
                ("RAM", s.synthetic_ram_score),
                ("Disk", s.synthetic_disk_score),
            ];
            let any_score = scores.iter().any(|(_, v)| v.is_some());
            if any_score {
                for (label, value) in scores {
                    let v_str = value
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "—".to_string());
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("     {label}: "),
                            Style::default().fg(theme.text_dim),
                        ),
                        Span::styled(v_str, Style::default().fg(theme.text)),
                    ]));
                }
            } else {
                lines.push(Line::from(Span::styled(
                    "     No baseline scores",
                    Style::default().fg(theme.muted),
                )));
            }
        }
        lines.push(Line::from(""));

        // Section 3: Game & Results
        let gr_toggle = if expanded[2] { "▼" } else { "►" };
        lines.push(Line::from(vec![
            Span::styled(format!(" {gr_toggle} "), Style::default().fg(theme.oracle)),
            Span::styled(
                "[3] Game & Results",
                Style::default()
                    .fg(theme.oracle)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        if expanded[2] {
            lines.push(Line::from(vec![
                Span::styled("     Game: ", Style::default().fg(theme.text_dim)),
                Span::styled(s.game.clone(), Style::default().fg(theme.text)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("     Resolution: ", Style::default().fg(theme.text_dim)),
                Span::styled(s.resolution.clone(), Style::default().fg(theme.text)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("     Preset: ", Style::default().fg(theme.text_dim)),
                Span::styled(s.preset.clone(), Style::default().fg(theme.text)),
            ]));

            let fps_color = theme.fps_color(s.avg_fps);
            lines.push(Line::from(vec![
                Span::styled("     Avg FPS: ", Style::default().fg(theme.text_dim)),
                Span::styled(
                    format!("{:.0}", s.avg_fps),
                    Style::default().fg(fps_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" ({})", Theme::fps_label(s.avg_fps)),
                    Style::default().fg(fps_color),
                ),
            ]));

            if let Some(low) = s.fps_1_low {
                lines.push(Line::from(vec![
                    Span::styled("     1% low: ", Style::default().fg(theme.text_dim)),
                    Span::styled(format!("{low:.0}"), Style::default().fg(theme.text)),
                ]));
            }
            if s.ray_tracing {
                lines.push(Line::from(vec![
                    Span::styled("     Ray tracing: ", Style::default().fg(theme.text_dim)),
                    Span::styled("ON", Style::default().fg(theme.optimal)),
                ]));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Press Enter to submit, Esc to go back.",
            Style::default().fg(theme.muted),
        )));
    }

    let body = Paragraph::new(Text::from(lines))
        .block(
            Block::default()
                .title(Span::styled(
                    " Review ",
                    Style::default()
                        .fg(theme.oracle)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(theme.border)),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(body, area);
}

pub(crate) fn draw_contribute_result(area: Rect, f: &mut ratatui::Frame, app: &App, theme: Theme) {
    let state = match &app.contribute.result_message {
        Some(s) => s,
        None => return,
    };

    let is_success = state.title.contains("Submitted") || state.title.contains("Queued");

    // Layout: wordmark banner + result body.
    // Wordmark is 4 lines tall; reserve 1 extra line so the drop shadow reads as "3D".
    let desired_banner_h = if area.width >= 54 { 5 } else { 1 };
    // Keep at least a small body visible even in short terminals.
    let min_body_h: u16 = 3;
    let max_banner_h = area.height.saturating_sub(min_body_h);
    let banner_h = desired_banner_h.min(max_banner_h);
    let chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Length(banner_h),
            ratatui::layout::Constraint::Min(0),
        ])
        .split(area);

    draw_wordmark(
        chunks[0],
        f,
        &theme,
        if is_success {
            theme.optimal
        } else {
            theme.oracle
        },
    );

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));

    // Title
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

    // Body
    for line in state.body.lines() {
        lines.push(Line::from(Span::styled(
            format!("  {line}"),
            Style::default().fg(theme.text),
        )));
    }

    // Status badge
    if is_success {
        lines.push(Line::from(""));
        let badge_text = if state.title.contains("Queued") {
            "Queued"
        } else {
            "Submitted"
        };
        lines.push(Line::from(vec![
            Span::styled("  [", Style::default().fg(theme.optimal)),
            Span::styled(
                badge_text,
                Style::default()
                    .fg(theme.optimal)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("]", Style::default().fg(theme.optimal)),
        ]));
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
    f.render_widget(para, chunks[1]);
}
