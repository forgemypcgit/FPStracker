use std::path::Path;

use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

pub(super) fn foreground_process_name_impl() -> Option<String> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_null() {
        return None;
    }

    let mut pid = 0u32;
    let _ = unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    if pid == 0 {
        return None;
    }

    process_name_from_pid(pid)
}

fn process_name_from_pid(pid: u32) -> Option<String> {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }

    let mut name_buf = vec![0u16; 1024];
    let mut size = name_buf.len() as u32;
    let ok = unsafe {
        QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, name_buf.as_mut_ptr(), &mut size)
    };

    let _ = unsafe { CloseHandle(handle) };

    if ok == 0 || size == 0 {
        return None;
    }

    let full_path = String::from_utf16_lossy(&name_buf[..size as usize]);
    let process = Path::new(full_path.trim())
        .file_stem()
        .and_then(|stem| stem.to_str())?;
    let trimmed = process.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
