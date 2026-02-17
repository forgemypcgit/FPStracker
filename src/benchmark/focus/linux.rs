use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

pub(super) fn foreground_process_name_impl() -> Option<String> {
    match *preferred_backend() {
        LinuxFocusBackend::Xdotool => foreground_pid_xdotool()
            .or_else(foreground_pid_xprop)
            .and_then(process_name_from_pid),
        LinuxFocusBackend::Xprop => foreground_pid_xprop().and_then(process_name_from_pid),
        LinuxFocusBackend::Hyprctl => foreground_pid_hyprctl().and_then(process_name_from_pid),
        LinuxFocusBackend::Unavailable => None,
    }
}

#[derive(Clone, Copy, Debug)]
enum LinuxFocusBackend {
    Xdotool,
    Xprop,
    Hyprctl,
    Unavailable,
}

static LINUX_BACKEND: OnceLock<LinuxFocusBackend> = OnceLock::new();

fn preferred_backend() -> &'static LinuxFocusBackend {
    LINUX_BACKEND.get_or_init(detect_backend)
}

fn detect_backend() -> LinuxFocusBackend {
    if command_exists("xdotool") {
        LinuxFocusBackend::Xdotool
    } else if command_exists("xprop") {
        LinuxFocusBackend::Xprop
    } else if command_exists("hyprctl") {
        LinuxFocusBackend::Hyprctl
    } else {
        LinuxFocusBackend::Unavailable
    }
}

fn command_exists(command: &str) -> bool {
    Command::new("which")
        .arg(command)
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn foreground_pid_xdotool() -> Option<u32> {
    let output = run_command_capture("xdotool", &["getwindowfocus", "getwindowpid"])?;
    output.trim().parse::<u32>().ok().filter(|pid| *pid > 0)
}

fn foreground_pid_xprop() -> Option<u32> {
    let root = run_command_capture("xprop", &["-root", "_NET_ACTIVE_WINDOW"])?;
    let window_id = root
        .split('#')
        .nth(1)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;

    let pid_line = run_command_capture("xprop", &["-id", window_id, "_NET_WM_PID"])?;
    parse_number_after_equals(&pid_line)
}

fn foreground_pid_hyprctl() -> Option<u32> {
    let output = run_command_capture("hyprctl", &["activewindow", "-j"])?;
    parse_hyprctl_pid(&output)
}

fn parse_hyprctl_pid(json: &str) -> Option<u32> {
    let marker = "\"pid\":";
    let idx = json.find(marker)?;
    let mut digits = String::new();
    for ch in json[idx + marker.len()..].chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else if !digits.is_empty() {
            break;
        }
    }
    digits.parse::<u32>().ok().filter(|pid| *pid > 0)
}

fn parse_number_after_equals(text: &str) -> Option<u32> {
    text.split('=')
        .nth(1)
        .map(str::trim)
        .and_then(|value| value.split_whitespace().next())
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|pid| *pid > 0)
}

fn process_name_from_pid(pid: u32) -> Option<String> {
    let comm_path = format!("/proc/{pid}/comm");
    if let Ok(comm) = fs::read_to_string(comm_path) {
        let trimmed = comm.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    let cmdline_path = format!("/proc/{pid}/cmdline");
    let cmdline = fs::read(cmdline_path).ok()?;
    let first = cmdline
        .split(|byte| *byte == 0)
        .next()
        .filter(|value| !value.is_empty())?;
    let command = String::from_utf8_lossy(first);
    let binary = Path::new(command.as_ref())
        .file_name()
        .and_then(|name| name.to_str())?;
    let trimmed = binary.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
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
