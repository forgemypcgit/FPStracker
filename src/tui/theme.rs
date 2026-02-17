//! TUI color theme.

use ratatui::style::Color;

use crate::games::GameDifficulty;

#[derive(Clone, Copy)]
pub(crate) struct Theme {
    // Primary palette
    pub oracle: Color,
    pub optimal: Color,
    pub caution: Color,
    pub critical: Color,

    // UI chrome
    pub border: Color,
    pub muted: Color,
    pub text: Color,
    pub text_dim: Color,

    // Backward-compat aliases
    pub good: Color,
    pub warning: Color,
}

impl Default for Theme {
    fn default() -> Self {
        let oracle = Color::Rgb(0, 212, 255);
        let optimal = Color::Rgb(163, 230, 53);
        let caution = Color::Rgb(251, 191, 36);
        let critical = Color::Rgb(255, 68, 85);

        Self {
            oracle,
            optimal,
            caution,
            critical,
            border: Color::Gray,
            muted: Color::DarkGray,
            text: Color::White,
            text_dim: Color::Gray,
            good: optimal,
            warning: caution,
        }
    }
}

impl Theme {
    pub fn tier_color(&self, difficulty: GameDifficulty) -> Color {
        match difficulty {
            GameDifficulty::Extreme => self.critical,
            GameDifficulty::Heavy => self.caution,
            GameDifficulty::Medium => self.oracle,
            GameDifficulty::Light => self.optimal,
        }
    }

    pub fn fps_color(&self, fps: f64) -> Color {
        if fps >= 120.0 {
            self.optimal
        } else if fps >= 60.0 {
            self.oracle
        } else if fps >= 30.0 {
            self.caution
        } else {
            self.critical
        }
    }

    pub fn fps_label(fps: f64) -> &'static str {
        if fps >= 120.0 {
            "Excellent"
        } else if fps >= 60.0 {
            "Smooth"
        } else if fps >= 30.0 {
            "Playable"
        } else {
            "Struggling"
        }
    }
}
