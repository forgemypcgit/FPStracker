//! Known games database
//!
//! This database helps users understand which games are good for benchmarking
//! and provides consistent benchmark settings recommendations.

use serde::{Deserialize, Serialize};

/// How demanding is the game on GPU
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameDifficulty {
    /// Lightweight games (e.g., CS2, Valorant, LoL)
    Light,
    /// Medium difficulty (e.g., Fortnite, Apex Legends)
    Medium,
    /// Heavy games (e.g., Cyberpunk 2077, RDR2)
    Heavy,
    /// Extremely demanding (e.g., Avatar: Frontiers of Pandora)
    Extreme,
}

impl std::fmt::Display for GameDifficulty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameDifficulty::Light => write!(f, "Light"),
            GameDifficulty::Medium => write!(f, "Medium"),
            GameDifficulty::Heavy => write!(f, "Heavy"),
            GameDifficulty::Extreme => write!(f, "Extreme"),
        }
    }
}

/// Information about a known game
#[derive(Debug, Clone, Serialize)]
pub struct GameInfo {
    /// Game name (canonical form)
    pub name: &'static str,
    /// Alternative names/abbreviations
    pub aliases: &'static [&'static str],
    /// GPU difficulty rating
    pub difficulty: GameDifficulty,
    /// Has built-in benchmark
    pub has_benchmark: bool,
    /// Supports ray tracing
    pub supports_rt: bool,
    /// Supports DLSS
    pub supports_dlss: bool,
    /// Supports FSR
    pub supports_fsr: bool,
    /// Recommended benchmark location/method
    pub benchmark_notes: &'static str,
}

/// Database of known games with benchmark difficulty ratings
pub static KNOWN_GAMES: &[GameInfo] = &[
    // ============ EXTREME (90+ percentile GPU load) ============
    GameInfo {
        name: "Cyberpunk 2077",
        aliases: &["CP2077", "Cyberpunk"],
        difficulty: GameDifficulty::Extreme,
        has_benchmark: true,
        supports_rt: true,
        supports_dlss: true,
        supports_fsr: true,
        benchmark_notes: "Use built-in benchmark. RT Overdrive mode for extreme testing.",
    },
    GameInfo {
        name: "Avatar: Frontiers of Pandora",
        aliases: &["Avatar", "AFOP"],
        difficulty: GameDifficulty::Extreme,
        has_benchmark: true,
        supports_rt: true,
        supports_dlss: true,
        supports_fsr: true,
        benchmark_notes: "Use built-in benchmark.",
    },
    GameInfo {
        name: "Alan Wake 2",
        aliases: &["AW2"],
        difficulty: GameDifficulty::Extreme,
        has_benchmark: false,
        supports_rt: true,
        supports_dlss: true,
        supports_fsr: true,
        benchmark_notes: "Test in Bright Falls town area for consistent load.",
    },
    GameInfo {
        name: "Hogwarts Legacy",
        aliases: &["Hogwarts"],
        difficulty: GameDifficulty::Extreme,
        has_benchmark: true,
        supports_rt: true,
        supports_dlss: true,
        supports_fsr: true,
        benchmark_notes: "Use built-in benchmark.",
    },
    GameInfo {
        name: "Black Myth: Wukong",
        aliases: &["Wukong", "BMW"],
        difficulty: GameDifficulty::Extreme,
        has_benchmark: true,
        supports_rt: true,
        supports_dlss: true,
        supports_fsr: true,
        benchmark_notes: "Use built-in benchmark.",
    },

    // ============ HEAVY (75-90 percentile GPU load) ============
    GameInfo {
        name: "Red Dead Redemption 2",
        aliases: &["RDR2", "Red Dead 2"],
        difficulty: GameDifficulty::Heavy,
        has_benchmark: true,
        supports_rt: false,
        supports_dlss: true,
        supports_fsr: false,
        benchmark_notes: "Use built-in benchmark (all 5 scenes).",
    },
    GameInfo {
        name: "The Witcher 3",
        aliases: &["Witcher 3", "TW3"],
        difficulty: GameDifficulty::Heavy,
        has_benchmark: false,
        supports_rt: true,
        supports_dlss: true,
        supports_fsr: true,
        benchmark_notes: "Test in Novigrad city for high NPC density.",
    },
    GameInfo {
        name: "Microsoft Flight Simulator 2024",
        aliases: &["MSFS", "Flight Sim", "MSFS2024"],
        difficulty: GameDifficulty::Heavy,
        has_benchmark: false,
        supports_rt: false,
        supports_dlss: true,
        supports_fsr: true,
        benchmark_notes: "Test landing at major airports (JFK, LHR).",
    },
    GameInfo {
        name: "Starfield",
        aliases: &["SF"],
        difficulty: GameDifficulty::Heavy,
        has_benchmark: false,
        supports_rt: false,
        supports_dlss: true,
        supports_fsr: true,
        benchmark_notes: "Test in New Atlantis city for consistent load.",
    },
    GameInfo {
        name: "Dying Light 2",
        aliases: &["DL2"],
        difficulty: GameDifficulty::Heavy,
        has_benchmark: true,
        supports_rt: true,
        supports_dlss: true,
        supports_fsr: true,
        benchmark_notes: "Use built-in benchmark.",
    },
    GameInfo {
        name: "Horizon Zero Dawn",
        aliases: &["HZD", "Horizon"],
        difficulty: GameDifficulty::Heavy,
        has_benchmark: true,
        supports_rt: false,
        supports_dlss: true,
        supports_fsr: true,
        benchmark_notes: "Use built-in benchmark.",
    },
    GameInfo {
        name: "Horizon Forbidden West",
        aliases: &["HFW"],
        difficulty: GameDifficulty::Heavy,
        has_benchmark: true,
        supports_rt: false,
        supports_dlss: true,
        supports_fsr: true,
        benchmark_notes: "Use built-in benchmark.",
    },

    // ============ MEDIUM (50-75 percentile GPU load) ============
    GameInfo {
        name: "Fortnite",
        aliases: &["FN"],
        difficulty: GameDifficulty::Medium,
        has_benchmark: false,
        supports_rt: true,
        supports_dlss: true,
        supports_fsr: true,
        benchmark_notes: "Land at Tilted and fight for 2 minutes.",
    },
    GameInfo {
        name: "Apex Legends",
        aliases: &["Apex"],
        difficulty: GameDifficulty::Medium,
        has_benchmark: false,
        supports_rt: false,
        supports_dlss: false,
        supports_fsr: false,
        benchmark_notes: "Drop hot and test during first fight.",
    },
    GameInfo {
        name: "Call of Duty: Warzone",
        aliases: &["Warzone", "WZ", "COD Warzone"],
        difficulty: GameDifficulty::Medium,
        has_benchmark: false,
        supports_rt: true,
        supports_dlss: true,
        supports_fsr: true,
        benchmark_notes: "Test in Verdansk/Caldera while moving.",
    },
    GameInfo {
        name: "Elden Ring",
        aliases: &["ER"],
        difficulty: GameDifficulty::Medium,
        has_benchmark: false,
        supports_rt: true,
        supports_dlss: false,
        supports_fsr: false,
        benchmark_notes: "Test in Limgrave open world.",
    },
    GameInfo {
        name: "Monster Hunter Wilds",
        aliases: &["MHW", "MH Wilds"],
        difficulty: GameDifficulty::Medium,
        has_benchmark: false,
        supports_rt: true,
        supports_dlss: true,
        supports_fsr: true,
        benchmark_notes: "Test during large monster fights.",
    },
    GameInfo {
        name: "Baldur's Gate 3",
        aliases: &["BG3"],
        difficulty: GameDifficulty::Medium,
        has_benchmark: false,
        supports_rt: false,
        supports_dlss: true,
        supports_fsr: true,
        benchmark_notes: "Test in Act 3 city areas.",
    },

    // ============ LIGHT (< 50 percentile GPU load) ============
    GameInfo {
        name: "Counter-Strike 2",
        aliases: &["CS2", "CS", "Counter-Strike"],
        difficulty: GameDifficulty::Light,
        has_benchmark: false,
        supports_rt: false,
        supports_dlss: false,
        supports_fsr: true,
        benchmark_notes: "Test on de_dust2 with bots.",
    },
    GameInfo {
        name: "Valorant",
        aliases: &["Val"],
        difficulty: GameDifficulty::Light,
        has_benchmark: false,
        supports_rt: false,
        supports_dlss: false,
        supports_fsr: false,
        benchmark_notes: "Test in deathmatch mode.",
    },
    GameInfo {
        name: "League of Legends",
        aliases: &["LoL", "League"],
        difficulty: GameDifficulty::Light,
        has_benchmark: false,
        supports_rt: false,
        supports_dlss: false,
        supports_fsr: false,
        benchmark_notes: "Test during 5v5 teamfight.",
    },
    GameInfo {
        name: "Dota 2",
        aliases: &["Dota"],
        difficulty: GameDifficulty::Light,
        has_benchmark: false,
        supports_rt: false,
        supports_dlss: false,
        supports_fsr: false,
        benchmark_notes: "Test during large teamfight.",
    },
    GameInfo {
        name: "Minecraft Java",
        aliases: &["MC Java", "Minecraft"],
        difficulty: GameDifficulty::Light,
        has_benchmark: false,
        supports_rt: false,
        supports_dlss: false,
        supports_fsr: false,
        benchmark_notes: "Java Edition: Test in a world with many chunks loaded. Use shader packs like SEUS PTGI for heavy GPU load.",
    },
    GameInfo {
        name: "Minecraft Bedrock (RTX)",
        aliases: &["MC Bedrock", "Minecraft RTX"],
        difficulty: GameDifficulty::Extreme,
        has_benchmark: true,
        supports_rt: true,
        supports_dlss: true,
        supports_fsr: false,
        benchmark_notes: "Bedrock RTX: Use built-in RTX worlds like Colosseum or Dungeon Dash. Requires RTX GPU.",
    },
    GameInfo {
        name: "Overwatch 2",
        aliases: &["OW2", "Overwatch"],
        difficulty: GameDifficulty::Light,
        has_benchmark: false,
        supports_rt: false,
        supports_dlss: false,
        supports_fsr: true,
        benchmark_notes: "Test in quick play match.",
    },
];

impl GameInfo {
    /// Find a game by name or alias (case-insensitive)
    pub fn find(query: &str) -> Option<&'static GameInfo> {
        let query_lower = query.to_lowercase();
        let query_normalized = normalize_match_key(query);
        KNOWN_GAMES.iter().find(|g| {
            g.name.to_lowercase() == query_lower
                || g.aliases.iter().any(|a| a.to_lowercase() == query_lower)
                || normalize_match_key(g.name) == query_normalized
                || g.aliases
                    .iter()
                    .any(|a| normalize_match_key(a) == query_normalized)
        })
    }

    /// Get games by difficulty
    #[allow(dead_code)]
    pub fn by_difficulty(difficulty: GameDifficulty) -> Vec<&'static GameInfo> {
        KNOWN_GAMES
            .iter()
            .filter(|g| g.difficulty == difficulty)
            .collect()
    }

    /// Suggested executable/process names for safer external capture targeting.
    /// The first entry is the default hint used for auto-selection.
    pub fn process_name_suggestions(&self) -> &'static [&'static str] {
        match self.name {
            "Cyberpunk 2077" => &["Cyberpunk2077.exe", "Cyberpunk2077", "Cyberpunk2077GOG.exe"],
            "Avatar: Frontiers of Pandora" => &["AFOP.exe", "AvatarFrontiersOfPandora.exe", "AFOP"],
            "Alan Wake 2" => &["AlanWake2.exe", "AlanWake2"],
            "Hogwarts Legacy" => &["HogwartsLegacy.exe", "HogwartsLegacy"],
            "Black Myth: Wukong" => &["BlackMythWukong.exe", "b1.exe", "BlackMythWukong"],
            "Red Dead Redemption 2" => &["RDR2.exe", "RDR2"],
            "The Witcher 3" => &["witcher3.exe", "witcher3", "witcher3_x64.exe"],
            "Microsoft Flight Simulator 2024" => &[
                "FlightSimulator.exe",
                "FlightSimulator",
                "Microsoft Flight Simulator",
            ],
            "Starfield" => &["Starfield.exe", "Starfield"],
            "Dying Light 2" => &[
                "DyingLightGame_x64_rwdi.exe",
                "DyingLightGame_x64_rwdi",
                "DyingLight2",
            ],
            "Horizon Zero Dawn" => &["HorizonZeroDawn.exe", "HorizonZeroDawn"],
            "Horizon Forbidden West" => &["HorizonForbiddenWest.exe", "HorizonForbiddenWest"],
            "Fortnite" => &[
                "FortniteClient-Win64-Shipping.exe",
                "FortniteClient-Win64-Shipping",
                "FortniteLauncher.exe",
            ],
            "Apex Legends" => &["r5apex.exe", "r5apex"],
            "Call of Duty: Warzone" => &["cod.exe", "ModernWarfare.exe", "cod", "iw8"],
            "Elden Ring" => &["eldenring.exe", "eldenring"],
            "Monster Hunter Wilds" => &["MonsterHunterWilds.exe", "MonsterHunterWilds"],
            "Baldur's Gate 3" => &["bg3.exe", "bg3_dx11.exe", "bg3", "bg3_dx11"],
            "Counter-Strike 2" => &["cs2.exe", "cs2", "hl2_linux"],
            "Valorant" => &[
                "VALORANT-Win64-Shipping.exe",
                "VALORANT-Win64-Shipping",
                "VALORANT",
            ],
            "League of Legends" => &[
                "League of Legends.exe",
                "LeagueClient.exe",
                "LeagueofLegends",
            ],
            "Dota 2" => &["dota2.exe", "dota2", "dota2_linux"],
            "Minecraft Java" => &["javaw.exe", "java.exe", "javaw", "java"],
            "Minecraft Bedrock (RTX)" => &["Minecraft.Windows.exe", "Minecraft.Windows"],
            "Overwatch 2" => &["Overwatch.exe", "Overwatch", "Overwatch2"],
            _ => &[],
        }
    }
}

fn normalize_match_key(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::GameInfo;

    #[test]
    fn find_supports_normalized_queries() {
        let game = GameInfo::find("counter strike 2").expect("Expected normalized match");
        assert_eq!(game.name, "Counter-Strike 2");
    }

    #[test]
    fn process_suggestions_include_platform_variants() {
        let game = GameInfo::find("Dota 2").expect("Expected known game");
        let suggestions = game.process_name_suggestions();
        assert!(suggestions.contains(&"dota2.exe"));
        assert!(suggestions.contains(&"dota2_linux"));
    }
}
