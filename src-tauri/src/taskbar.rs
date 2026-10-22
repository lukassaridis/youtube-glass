//! Folds WebView2's native picture-in-picture window into the app's taskbar group.
//!
//! Chromium PiP is a top-level window owned by the msedgewebview2.exe browser process.
//! WebView2 stamps it with the host's explicit AppUserModelID (so it groups with us once
//! `set_app_id` has run), but it still carries no icon of its own, so we hand it ours.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Manager};
use windows::core::{BOOL, HSTRING};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassLongPtrW, GetWindow, GetWindowLongPtrW, GetWindowThreadProcessId,
    IsWindowVisible, PostMessageW, SendMessageW, GCLP_HICON, GCLP_HICONSM, GET_CLASS_LONG_INDEX,
    GWL_EXSTYLE, GW_OWNER, ICON_BIG, ICON_SMALL, WM_GETICON, WM_SETICON, WS_EX_TOOLWINDOW,
};

const PIP_POLL_ATTEMPTS: u32 = 25;
const PIP_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Gives the process an explicit AppUserModelID matching the installer's shortcuts.
/// Must run before any window is shown.
pub fn set_app_id(app_id: &str) {
    unsafe {
        let _ = SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(app_id));
    }
}

pub fn adopt_native_pip(app: &AppHandle) {
    let Some(youtube) = app.get_webview("youtube") else {
        return;
    };
    let Some(main) = app.get_window("main") else {
        return;
    };
    let Ok(main_hwnd) = main.hwnd() else {
        return;
    };
    let main_hwnd = main_hwnd.0 as isize;

    let (tx, rx) = mpsc::channel();
    let sent = youtube.with_webview(move |webview| {
        let mut pid = 0u32;
        unsafe {
            if let Ok(core) = webview.controller().CoreWebView2() {
                let _ = core.BrowserProcessId(&mut pid);
            }
        }
        let _ = tx.send(pid);
    });
    if sent.is_err() {
        return;
    }

    thread::spawn(move || {
        let Ok(browser_pid) = rx.recv_timeout(Duration::from_secs(2)) else {
            return;
        };
        if browser_pid == 0 {
            return;
        }
        for _ in 0..PIP_POLL_ATTEMPTS {
            let windows = taskbar_windows_of(browser_pid);
            if !windows.is_empty() {
                let main_hwnd = HWND(main_hwnd as _);
                for hwnd in windows {
                    copy_icon(HWND(hwnd as _), main_hwnd);
                }
                return;
            }
            thread::sleep(PIP_POLL_INTERVAL);
        }
    });
}

struct WindowSearch {
    pid: u32,
    found: Vec<isize>,
}

unsafe extern "system" fn collect_taskbar_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let search = &mut *(lparam.0 as *mut WindowSearch);
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid != search.pid || !IsWindowVisible(hwnd).as_bool() {
        return true.into();
    }
    let owned = GetWindow(hwnd, GW_OWNER).is_ok_and(|owner| !owner.is_invalid());
    let tool = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 != 0;
    if !owned && !tool {
        search.found.push(hwnd.0 as isize);
    }
    true.into()
}

/// Visible, unowned, non-tool top-level windows of `pid`: the ones that get taskbar buttons.
/// WebView2 content HWNDs are children of our window, so this only matches PiP.
fn taskbar_windows_of(pid: u32) -> Vec<isize> {
    let mut search = WindowSearch {
        pid,
        found: Vec::new(),
    };
    unsafe {
        let _ = EnumWindows(
            Some(collect_taskbar_window),
            LPARAM(&mut search as *mut WindowSearch as isize),
        );
    }
    search.found
}

fn icon_of(hwnd: HWND, kind: u32, class_index: GET_CLASS_LONG_INDEX) -> isize {
    unsafe {
        let icon = SendMessageW(hwnd, WM_GETICON, Some(WPARAM(kind as usize)), None).0;
        if icon != 0 {
            icon
        } else {
            GetClassLongPtrW(hwnd, class_index) as isize
        }
    }
}

/// Gives the PiP window the main window's icon. HICONs are session-wide, so posting
/// WM_SETICON across processes is fine.
fn copy_icon(hwnd: HWND, main_hwnd: HWND) {
    let small = icon_of(main_hwnd, ICON_SMALL, GCLP_HICONSM);
    let big = icon_of(main_hwnd, ICON_BIG, GCLP_HICON);
    for (kind, icon) in [
        (ICON_SMALL, if small != 0 { small } else { big }),
        (ICON_BIG, if big != 0 { big } else { small }),
    ] {
        if icon != 0 {
            unsafe {
                let _ = PostMessageW(Some(hwnd), WM_SETICON, WPARAM(kind as usize), LPARAM(icon));
            }
        }
    }
}
