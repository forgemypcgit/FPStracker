//! Home screen drawing.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Wrap};

use crate::tui::state::{App, HomeChoice};
use crate::tui::theme::Theme;
use crate::tui::widgets::banner::draw_banner;
use crate::tui::widgets::footer::draw_footer;

pub(crate) fn draw_home(area: Rect, f: &mut ratatui::Frame, app: &App, theme: Theme) {
    // Keep the menu usable even in short terminals by shrinking the banner/tagline first.
    let min_menu_h: u16 = 3;
    let footer_h: u16 = if area.height >= 1 { 1 } else { 0 };
    let mut banner_h: u16 = 5.min(area.height);
    let mut tagline_h: u16 = 2.min(area.height.saturating_sub(banner_h));

    // Ensure the menu has a chance to render (borders + at least one row).
    let target_fixed_max = area.height.saturating_sub(min_menu_h);
    while banner_h.saturating_add(tagline_h).saturating_add(footer_h) > target_fixed_max {
        if tagline_h > 0 {
            tagline_h = tagline_h.saturating_sub(1);
            continue;
        }
        if banner_h > 1 {
            banner_h = banner_h.saturating_sub(1);
            continue;
        }
        break;
    }

    // Optional Windows dependency hint (PresentMon).
    #[cfg(target_os = "windows")]
    let deps_hint_h: u16 = {
        let missing = crate::deps::locate_presentmon_executable().is_none();
        if missing {
            3
        } else {
            0
        }
    };
    #[cfg(not(target_os = "windows"))]
    let deps_hint_h: u16 = 0;

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(banner_h),
            Constraint::Length(tagline_h),
            Constraint::Length(deps_hint_h),
            Constraint::Min(0),
            Constraint::Length(footer_h),
        ])
        .split(area);

    // ASCII banner
    draw_banner(layout[0], f, &theme);

    // Tagline (optional in very short terminals)
    if layout[1].height > 0 {
        let tagline = Paragraph::new(Text::from(vec![Line::from(Span::styled(
            "Privacy-first benchmark collection. No injection. No accounts.",
            Style::default().fg(theme.text_dim),
        ))]))
        .wrap(Wrap { trim: true });
        f.render_widget(tagline, layout[1]);
    }

    #[cfg(target_os = "windows")]
    if layout[2].height > 0 {
        let installing = app.presentmon_install.is_some();
        let msg = if installing {
            format!(
                "{} Installing PresentMon... (this can take a minute)",
                app.animation.spinner_char()
            )
        } else {
            "PresentMon not found. Press I to install (recommended for Windows live capture)."
                .to_string()
        };
        let hint = Paragraph::new(Text::from(vec![Line::from(Span::styled(
            msg,
            Style::default().fg(theme.caution),
        ))]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(theme.border)),
        )
        .wrap(Wrap { trim: true });
        f.render_widget(hint, layout[2]);
    }

    // Menu items with descriptions
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    let menu: Vec<(HomeChoice, &str, &str)> = vec![
        (
            HomeChoice::GuidedFlow,
            "Contribute benchmark",
            "Submit GPU/CPU specs + FPS data for a game",
        ),
        (
            HomeChoice::Synthetic,
            "Run synthetic baseline",
            "Benchmark CPU/GPU/RAM/Disk locally",
        ),
        (
            HomeChoice::Feedback,
            "Send feedback",
            "Report bugs or suggest improvements",
        ),
        (HomeChoice::Quit, "Quit", "Exit the application"),
    ];

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    let menu: Vec<(HomeChoice, &str, &str)> = vec![
        (
            HomeChoice::GuidedFlow,
            "Contribute benchmark",
            "Submit GPU/CPU specs + FPS data for a game",
        ),
        (
            HomeChoice::Feedback,
            "Send feedback",
            "Report bugs or suggest improvements",
        ),
        (HomeChoice::Quit, "Quit", "Exit the application"),
    ];

    let items = menu
        .into_iter()
        .map(|(choice, label, desc)| {
            let selected = choice == app.home_choice;
            let label_style = if selected {
                Style::default()
                    .fg(theme.oracle)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text)
            };
            let desc_style = Style::default().fg(theme.muted);
            let selector = if selected { "›" } else { " " };

            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(format!(" {selector} "), label_style),
                    Span::styled(label, label_style),
                ]),
                Line::from(vec![Span::raw("     "), Span::styled(desc, desc_style)]),
            ])
        })
        .collect::<Vec<_>>();

    let menu_widget = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.border)),
    );
    f.render_widget(menu_widget, layout[3]);

    // Footer keybind hints (optional in very short terminals)
    if layout[4].height > 0 {
        #[cfg(target_os = "windows")]
        let presentmon_missing = crate::deps::locate_presentmon_executable().is_none();

        #[cfg(target_os = "windows")]
        let hints: Vec<(&str, &str)> = if presentmon_missing {
            vec![
                ("↑/↓", "Navigate"),
                ("Enter", "Select"),
                ("I", "Install PM"),
                ("F", "Feedback"),
                ("Esc", "Quit"),
            ]
        } else {
            vec![
                ("↑/↓", "Navigate"),
                ("Enter", "Select"),
                ("F", "Feedback"),
                ("Esc", "Quit"),
            ]
        };
        #[cfg(not(target_os = "windows"))]
        let hints: Vec<(&str, &str)> = vec![
            ("↑/↓", "Navigate"),
            ("Enter", "Select"),
            ("F", "Feedback"),
            ("Esc", "Quit"),
        ];

        draw_footer(layout[4], f, &theme, &hints);
    }
}
