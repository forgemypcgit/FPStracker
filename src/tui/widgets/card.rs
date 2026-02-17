//! Rounded bordered panel widget.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};

use crate::tui::theme::Theme;

pub(crate) struct CardWidget<'a> {
    pub title: &'a str,
    pub lines: Vec<Line<'a>>,
    pub badge: Option<(&'a str, ratatui::style::Color)>,
    pub border_color: Option<ratatui::style::Color>,
}

impl<'a> CardWidget<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            lines: Vec::new(),
            badge: None,
            border_color: None,
        }
    }

    pub fn line(mut self, line: Line<'a>) -> Self {
        self.lines.push(line);
        self
    }

    pub fn badge(mut self, label: &'a str, color: ratatui::style::Color) -> Self {
        self.badge = Some((label, color));
        self
    }

    pub fn border_color(mut self, color: ratatui::style::Color) -> Self {
        self.border_color = Some(color);
        self
    }

    pub fn render(self, area: Rect, f: &mut ratatui::Frame, theme: &Theme) {
        let border_col = self.border_color.unwrap_or(theme.border);

        let mut title_spans = vec![Span::styled(
            self.title,
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        )];

        if let Some((badge_text, badge_color)) = self.badge {
            title_spans.push(Span::raw("  "));
            title_spans.push(Span::styled(
                badge_text,
                Style::default()
                    .fg(badge_color)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        let block = Block::default()
            .title(Line::from(title_spans))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_col));

        let para = Paragraph::new(Text::from(self.lines))
            .block(block)
            .wrap(Wrap { trim: true });
        f.render_widget(para, area);
    }
}
