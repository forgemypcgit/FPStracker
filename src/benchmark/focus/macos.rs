use std::path::Path;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use super::normalize_process_name;

pub(super) fn foreground_process_name_impl() -> Option<String> {
    if let Some(pid) = foreground_pid() {
        if let Some(cached) = cached_process_name(pid) {
            return Some(cached);
        }

        let resolved = process_name_from_pid(pid).or_else(foreground_app_name)?;
        update_cached_process_name(pid, &resolved);
        return Some(resolved);
    }

    foreground_app_name()
}

fn foreground_pid() -> Option<u32> {
    let script = r#"tell application "System Events" to get unix id of first process whose frontmost is true"#;
    let output = run_command_capture("osascript", &["-e", script])?;
    output.trim().parse::<u32>().ok().filter(|pid| *pid > 0)
}

fn process_name_from_pid(pid: u32) -> Option<String> {
    let pid_text = pid.to_string();
    let output = run_command_capture("ps", &["-p", &pid_text, "-o", "comm="])?;
    let command = output.trim();
    if command.is_empty() {
        return None;
    }
    let name = Path::new(command)
        .file_stem()
        .and_then(|value| value.to_str())?;
    let trimmed = name.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn foreground_app_name() -> Option<String> {
    let script =
        r#"tell application "System Events" to get name of first process whose frontmost is true"#;
    let output = run_command_capture("osascript", &["-e", script])?;
    let normalized = normalize_process_name(output.trim());
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

static PROCESS_NAME_CACHE: OnceLock<Mutex<Option<(u32, String)>>> = OnceLock::new();

fn cached_process_name(pid: u32) -> Option<String> {
    let cache = PROCESS_NAME_CACHE.get_or_init(|| Mutex::new(None));
    let guard = cache.lock().ok()?;
    match guard.as_ref() {
        Some((cached_pid, name)) if *cached_pid == pid => Some(name.clone()),
        _ => None,
    }
}

fn update_cached_process_name(pid: u32, name: &str) {
    let cache = PROCESS_NAME_CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = cache.lock() {
        *guard = Some((pid, name.to_string()));
    }
}

fn run_command_capture(command: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(command).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
