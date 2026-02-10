//! Games database module
//!
//! Contains known games with their GPU difficulty ratings and recommended
//! benchmark settings for consistent data collection.

mod database;

pub use database::{GameDifficulty, GameInfo, KNOWN_GAMES};
