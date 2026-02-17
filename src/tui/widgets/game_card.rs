//! Game list item with tier coloring and badges.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::ListItem;

use crate::games::{GameDifficulty, GameInfo};
use crate::tui::theme::Theme;

pub(crate) fn game_list_item<'a>(
    game: &'a GameInfo,
    selected: bool,
    theme: &Theme,
) -> ListItem<'a> {
    let tier_color = theme.tier_color(game.difficulty);

    let name_style = if selected {
        Style::default()
            .fg(theme.oracle)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text)
    };

    let mut spans = vec![
        Span::styled("▌", Style::default().fg(tier_color)),
        Span::styled(if selected { " › " } else { "   " }, name_style),
        Span::styled(game.name, name_style),
    ];

    // Benchmark badge
    if game.has_benchmark {
        spans.push(Span::raw(" "));
        spans.push(Span::styled("[Bench]", Style::default().fg(theme.optimal)));
    }

    // Difficulty badge
    let diff_label = match game.difficulty {
        GameDifficulty::Extreme => Some("[Extreme]"),
        GameDifficulty::Heavy => Some("[Heavy]"),
        _ => None,
    };
    if let Some(label) = diff_label {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(label, Style::default().fg(tier_color)));
    }

    // Anti-cheat risk badge
    let ac_risk = anti_cheat_risk(game.name);
    match ac_risk {
        AcRisk::High => {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                "[AC Risk]",
                Style::default().fg(theme.critical),
            ));
        }
        AcRisk::Medium => {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                "[AC Caution]",
                Style::default().fg(theme.caution),
            ));
        }
        AcRisk::Low => {}
    }

    ListItem::new(Line::from(spans))
}

enum AcRisk {
    Low,
    Medium,
    High,
}

fn anti_cheat_risk(name: &str) -> AcRisk {
    match name {
        "Valorant" | "League of Legends" => AcRisk::High,
        "Counter-Strike 2" | "Fortnite" | "Apex Legends" | "Call of Duty: Warzone" => {
            AcRisk::Medium
        }
        _ => AcRisk::Low,
    }
}
