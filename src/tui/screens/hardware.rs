//! Hardware screen drawing with spec cards.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::tui::state::{App, HardwareField, InputMode};
use crate::tui::theme::Theme;
use crate::tui::widgets::card::CardWidget;

pub(crate) fn draw_contribute_hardware(
    area: Rect,
    f: &mut ratatui::Frame,
    app: &App,
    theme: Theme,
) {
    let hw = app.contribute.hardware.as_ref();

    // Detecting spinner
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    if app.contribute.detect.is_some() {
        let spinner = app.animation.spinner_char();
        let elapsed = app
            .contribute
            .detect
            .as_ref()
            .map(|d| d.started_at.elapsed())
            .unwrap_or_default();

        let card = CardWidget::new("Detecting Hardware")
            .border_color(theme.oracle)
            .line(Line::from(""))
            .line(Line::from(vec![
                Span::styled(format!("  {spinner}  "), Style::default().fg(theme.oracle)),
                Span::styled(
                    format!("Scanning system... ({:.1}s)", elapsed.as_secs_f64()),
                    Style::default().fg(theme.text),
                ),
            ]))
            .line(Line::from(""))
            .line(Line::from(Span::styled(
                "  This may take a few seconds.",
                Style::default().fg(theme.muted),
            )));
        card.render(area, f, &theme);
        return;
    }

    if let Some(hw) = hw {
        // Layout: 3 spec cards + status
        let use_side_by_side = area.width >= 100;

        if use_side_by_side {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(6), // GPU + CPU side by side
                    Constraint::Length(5), // RAM
                    Constraint::Min(0),    // Status
                ])
                .split(area);

            let top_row = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(chunks[0]);

            draw_gpu_card(top_row[0], f, hw, &theme);
            draw_cpu_card(top_row[1], f, hw, &theme);
            draw_ram_card(chunks[1], f, hw, &theme);
            draw_hw_status(chunks[2], f, hw, &theme);
        } else {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(5), // GPU
                    Constraint::Length(6), // CPU
                    Constraint::Length(4), // RAM
                    Constraint::Min(0),    // Status
                ])
                .split(area);

            draw_gpu_card(chunks[0], f, hw, &theme);
            draw_cpu_card(chunks[1], f, hw, &theme);
            draw_ram_card(chunks[2], f, hw, &theme);
            draw_hw_status(chunks[3], f, hw, &theme);
        }
    } else {
        // No hardware draft yet
        let card = CardWidget::new("Hardware Detection")
            .line(Line::from(""))
            .line(Line::from(Span::styled(
                "  Press D to auto-detect your hardware, or Enter to begin.",
                Style::default().fg(theme.text),
            )))
            .line(Line::from(Span::styled(
                "  If detection fails, you can type values manually.",
                Style::default().fg(theme.muted),
            )));
        card.render(area, f, &theme);
    }
}

fn draw_gpu_card(
    area: Rect,
    f: &mut ratatui::Frame,
    hw: &crate::tui::state::HardwareForm,
    theme: &Theme,
) {
    let badge = if hw.confirm_gpu {
        Some(("Confirmed", theme.optimal))
    } else {
        None
    };
    let border = if hw.confirm_gpu {
        theme.optimal
    } else {
        theme.border
    };

    let mut card = CardWidget::new("GPU").border_color(border);
    if let Some((label, color)) = badge {
        card = card.badge(label, color);
    }

    card = card.line(field_line(
        "Model",
        &hw.gpu_name,
        hw.field == HardwareField::GpuName,
        hw.mode,
        theme,
    ));
    card = card.line(field_line(
        "VRAM (MB)",
        &hw.gpu_vram_mb,
        hw.field == HardwareField::GpuVramMb,
        hw.mode,
        theme,
    ));

    card.render(area, f, theme);
}

fn draw_cpu_card(
    area: Rect,
    f: &mut ratatui::Frame,
    hw: &crate::tui::state::HardwareForm,
    theme: &Theme,
) {
    let badge = if hw.confirm_cpu {
        Some(("Confirmed", theme.optimal))
    } else {
        None
    };
    let border = if hw.confirm_cpu {
        theme.optimal
    } else {
        theme.border
    };

    let mut card = CardWidget::new("CPU").border_color(border);
    if let Some((label, color)) = badge {
        card = card.badge(label, color);
    }

    card = card.line(field_line(
        "Model",
        &hw.cpu_name,
        hw.field == HardwareField::CpuName,
        hw.mode,
        theme,
    ));
    card = card.line(field_line(
        "Cores",
        &hw.cpu_cores,
        hw.field == HardwareField::CpuCores,
        hw.mode,
        theme,
    ));
    card = card.line(field_line(
        "Threads",
        &hw.cpu_threads,
        hw.field == HardwareField::CpuThreads,
        hw.mode,
        theme,
    ));

    card.render(area, f, theme);
}

fn draw_ram_card(
    area: Rect,
    f: &mut ratatui::Frame,
    hw: &crate::tui::state::HardwareForm,
    theme: &Theme,
) {
    let badge = if hw.confirm_ram {
        Some(("Confirmed", theme.optimal))
    } else {
        None
    };
    let border = if hw.confirm_ram {
        theme.optimal
    } else {
        theme.border
    };

    let mut card = CardWidget::new("RAM").border_color(border);
    if let Some((label, color)) = badge {
        card = card.badge(label, color);
    }

    card = card.line(field_line(
        "Total (MB)",
        &hw.ram_total_mb,
        hw.field == HardwareField::RamTotalMb,
        hw.mode,
        theme,
    ));

    card.render(area, f, theme);
}

fn field_line<'a>(
    label: &'a str,
    value: &'a str,
    selected: bool,
    mode: InputMode,
    theme: &Theme,
) -> Line<'a> {
    let display = if value.trim().is_empty() {
        "—"
    } else {
        value
    };

    let label_style = if selected {
        Style::default()
            .fg(theme.oracle)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text_dim)
    };

    let value_style = if selected && mode == InputMode::Edit {
        Style::default()
            .fg(theme.oracle)
            .add_modifier(Modifier::UNDERLINED)
    } else if selected {
        Style::default()
            .fg(theme.oracle)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text)
    };

    let selector = if selected { "›" } else { " " };
    let edit_indicator = if selected && mode == InputMode::Edit {
        " [EDIT]"
    } else {
        ""
    };

    Line::from(vec![
        Span::styled(format!(" {selector} "), label_style),
        Span::styled(format!("{:<12}", label), label_style),
        Span::styled(display, value_style),
        Span::styled(edit_indicator, Style::default().fg(theme.oracle)),
    ])
}

fn draw_hw_status(
    area: Rect,
    f: &mut ratatui::Frame,
    hw: &crate::tui::state::HardwareForm,
    theme: &Theme,
) {
    use ratatui::widgets::Paragraph;

    let ready = hw.can_continue();
    let (msg, style) = if ready {
        (
            "Ready — press Enter to continue",
            Style::default()
                .fg(theme.optimal)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        (
            if !hw.has_required_values() {
                "Fill required fields, then confirm each section (G/C/R)"
            } else {
                "Press G, C, R to confirm each hardware section"
            },
            Style::default().fg(theme.caution),
        )
    };

    let status = Paragraph::new(Line::from(Span::styled(format!("  {msg}"), style)));
    f.render_widget(status, area);
}
