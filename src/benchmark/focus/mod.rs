//! Foreground window/process detection.
//!
//! This module uses best-effort platform-specific process lookup for the
//! currently focused window.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
use linux::foreground_process_name_impl;
#[cfg(target_os = "macos")]
use macos::foreground_process_name_impl;
#[cfg(target_os = "windows")]
use windows::foreground_process_name_impl;

/// Best-effort foreground process name.
pub fn foreground_process_name() -> Option<String> {
    foreground_process_name_impl()
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn foreground_process_name_impl() -> Option<String> {
    None
}

/// Normalize process names across platforms/extensions for matching.
pub fn normalize_process_name(name: &str) -> String {
    name.trim()
        .trim_matches('"')
        .trim_end_matches(".exe")
        .trim_end_matches(".app")
        .to_ascii_lowercase()
}

/// Compare process names using normalized forms.
pub fn process_name_matches(active: &str, target: &str) -> bool {
    normalize_process_name(active) == normalize_process_name(target)
}

#[cfg(test)]
mod tests {
    use super::{normalize_process_name, process_name_matches};

    #[test]
    fn normalize_strips_common_suffixes() {
        assert_eq!(normalize_process_name("cs2.exe"), "cs2");
        assert_eq!(normalize_process_name("\"Game.app\""), "game");
    }

    #[test]
    fn process_match_uses_normalized_forms() {
        assert!(process_name_matches("r5apex.exe", "r5apex"));
        assert!(process_name_matches("Cyberpunk2077", "Cyberpunk2077.exe"));
        assert!(!process_name_matches("cs2.exe", "valorant.exe"));
    }
}
