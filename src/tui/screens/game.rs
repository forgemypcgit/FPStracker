//! Game selection screen with tier-grouped items and badges.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, Paragraph};

use crate::games::KNOWN_GAMES;
use crate::tui::state::{filtered_games, App};
use crate::tui::theme::Theme;
use crate::tui::widgets::game_card::game_list_item;

pub(crate) fn draw_contribute_game(area: Rect, f: &mut ratatui::Frame, app: &App, theme: Theme) {
    let games = filtered_games(&app.contribute.game.query);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // search bar
            Constraint::Min(0),    // game list
        ])
        .split(area);

    // Search bar
    let search_display = if app.contribute.game.query.is_empty() {
        Span::styled(
            "  > Type to search games...",
            Style::default().fg(theme.muted),
        )
    } else {
        Span::styled(
            format!("  > {}", app.contribute.game.query),
            Style::default()
                .fg(theme.oracle)
                .add_modifier(Modifier::BOLD),
        )
    };
    let search = Paragraph::new(Line::from(search_display));
    f.render_widget(search, chunks[0]);

    // Game list with tier coloring
    let max_rows = (chunks[1].height as usize).saturating_sub(2);

    // Scrolling: ensure cursor is visible
    let scroll_start = if app.contribute.game.cursor >= max_rows {
        app.contribute.game.cursor - max_rows + 1
    } else {
        0
    };

    let items: Vec<_> = games
        .iter()
        .copied()
        .skip(scroll_start)
        .take(max_rows)
        .enumerate()
        .map(|(pos, idx)| {
            let selected = (pos + scroll_start) == app.contribute.game.cursor;
            let g = &KNOWN_GAMES[idx];
            game_list_item(g, selected, &theme)
        })
        .collect();

    let count = games.len();
    let title = format!(" {} game{} ", count, if count == 1 { "" } else { "s" });

    let list = List::new(items).block(
        Block::default()
            .title(Span::styled(title, Style::default().fg(theme.text_dim)))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.border)),
    );
    f.render_widget(list, chunks[1]);
}
