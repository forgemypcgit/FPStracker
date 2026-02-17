//! ASCII art banner for the home screen.

use ratatui::layout::Alignment;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};

use crate::tui::theme::Theme;

// Simple, readable wordmark that we can safely show even in smaller terminals.
// We add a drop-shadow at render time to get a "3D" feel without needing a huge font.
//
// ASCII-only so this looks consistent across terminals (including Windows).
const WORDMARK_FULL: &[&str] = &[
    "  ___ ___ ___      _____ ___  ___  ___ _  _ ___ ___",
    " | __| _ \\ __|    |_   _| _ \\/ _ \\/ __| || | __| _ \\",
    " | _||  _/ _|       | | |   / (_) \\__ \\ __ | _||   /",
    " |_| |_| |___|      |_| |_|_\\\\___/|___/_||_|___|_|_\\",
];

const WORDMARK_COMPACT: &[&str] = &["FPS TRACKER"];

// Full wordmark is 52 columns wide; we require a little extra width so the
// drop shadow and centering don't feel cramped.
const WORDMARK_FULL_MIN_WIDTH: u16 = 54;
const WORDMARK_FULL_HEIGHT: u16 = 4;

pub(crate) fn draw_banner(area: Rect, f: &mut ratatui::Frame, theme: &Theme) {
    draw_wordmark(area, f, theme, theme.oracle);
}

pub(crate) fn draw_wordmark(
    area: Rect,
    f: &mut ratatui::Frame,
    theme: &Theme,
    color: ratatui::style::Color,
) {
    let use_full = area.width >= WORDMARK_FULL_MIN_WIDTH && area.height >= WORDMARK_FULL_HEIGHT;
    let raw_lines = if use_full {
        WORDMARK_FULL
    } else {
        WORDMARK_COMPACT
    };

    // Drop shadow: render the same lines offset by 1 row/col in a muted color.
    // This reads as "3D" without requiring an enormous ASCII font.
    let shadow_style = Style::default().fg(theme.text_dim);
    let main_style = Style::default().fg(color).add_modifier(Modifier::BOLD);

    // Ensure shorter wordmark lines never leave artifacts from prior frames.
    f.render_widget(Clear, area);

    if area.width >= 2 && area.height >= 2 {
        let shadow_area = Rect {
            x: area.x.saturating_add(1),
            y: area.y.saturating_add(1),
            width: area.width.saturating_sub(1),
            height: area.height.saturating_sub(1),
        };
        f.render_widget(Clear, shadow_area);
        let shadow_lines = raw_lines
            .iter()
            .take(shadow_area.height as usize)
            .map(|line| Line::from(Span::styled(*line, shadow_style)))
            .collect::<Vec<_>>();
        f.render_widget(
            Paragraph::new(shadow_lines).alignment(Alignment::Center),
            shadow_area,
        );
    }

    let lines = raw_lines
        .iter()
        .take(area.height as usize)
        .map(|line| Line::from(Span::styled(*line, main_style)))
        .collect::<Vec<_>>();
    f.render_widget(Paragraph::new(lines).alignment(Alignment::Center), area);
}
