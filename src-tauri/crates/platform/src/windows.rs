use std::path::Path;

use windows_sys::Win32::Foundation::{CloseHandle, HWND, POINT};
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetForegroundWindow, GetWindowThreadProcessId, IsWindow, SetForegroundWindow,
};

use crate::{DisplayArea, Session};

/// The previously focused top-level window, stored as an integer so the
/// handle can cross threads.
pub type Handle = isize;

pub fn session() -> Session {
    Session::Windows
}

pub fn foreground() -> Option<(Handle, Option<String>)> {
    // SAFETY: plain Win32 queries; every pointer passed is valid for the call.
    unsafe {
        let hwnd: HWND = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == std::process::id() {
            return None;
        }
        Some((hwnd as isize, process_name(pid)))
    }
}

unsafe fn process_name(pid: u32) -> Option<String> {
    let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
    if process.is_null() {
        return None;
    }
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    let ok = QueryFullProcessImageNameW(process, 0, buf.as_mut_ptr(), &mut len);
    CloseHandle(process);
    if ok == 0 {
        return None;
    }
    let path = String::from_utf16_lossy(&buf[..len as usize]);
    Path::new(&path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
}

pub fn activate(handle: &Handle) -> bool {
    // SAFETY: IsWindow guards against the window having been closed meanwhile.
    // We are the foreground process at this point, so Windows permits the call.
    unsafe {
        let hwnd = *handle as HWND;
        IsWindow(hwnd) != 0 && SetForegroundWindow(hwnd) != 0
    }
}

use windows_sys::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows_sys::Win32::UI::HiDpi::GetDpiForMonitor;

/// The work area of the monitor under the mouse pointer. Win32 reports
/// cursor and monitor rectangles in one shared physical pixel space, so the
/// hit-test needs no conversion.
pub fn cursor_display_work_area() -> Option<DisplayArea> {
    // SAFETY: plain Win32 queries; every pointer passed is valid for the call.
    unsafe {
        let mut point = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut point) == 0 {
            return None;
        }
        let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
        if monitor.is_null() {
            return None;
        }
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return None;
        }
        let mut dpi = 96u32;
        // MDT_EFFECTIVE_DPI; failing just means the fallback scale of 1.
        GetDpiForMonitor(monitor, 0, &mut dpi, &mut dpi);
        let work = info.rcWork;
        Some(DisplayArea {
            x: work.left as f64,
            y: work.top as f64,
            width: (work.right - work.left) as f64,
            height: (work.bottom - work.top) as f64,
            scale: dpi as f64 / 96.0,
            units_physical: true,
        })
    }
}
