//! Remembering which application was in front before the capture window
//! appeared, handing focus back to it afterwards, and telling which display
//! the mouse pointer is on so floating windows open where the user is.
//!
//! Each OS exposes these differently, and Wayland deliberately does not
//! expose them at all; see [`Session`].

#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod imp;
#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod imp;
#[cfg(windows)]
#[path = "windows.rs"]
mod imp;
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
#[path = "unsupported.rs"]
mod imp;

/// The application that had focus when capture was triggered.
#[derive(Debug, Clone)]
pub struct PreviousApp {
    /// Human-readable application name, when the OS provides one. Never a
    /// window title: titles routinely contain private text.
    pub name: Option<String>,
    handle: imp::Handle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Session {
    MacOs,
    Windows,
    X11,
    /// Wayland clients cannot see or activate other applications' windows.
    /// The compositor returns focus to the previous window by itself when
    /// ours is hidden, so nothing is lost; we just cannot influence it.
    Wayland,
    Unknown,
}

impl Session {
    pub fn as_str(self) -> &'static str {
        match self {
            Session::MacOs => "macos",
            Session::Windows => "windows",
            Session::X11 => "x11",
            Session::Wayland => "wayland",
            Session::Unknown => "unknown",
        }
    }
}

pub fn session() -> Session {
    imp::session()
}

/// Returns the currently focused application, or `None` when it is Posthaste
/// itself or the platform cannot tell.
pub fn foreground_app() -> Option<PreviousApp> {
    let (handle, name) = imp::foreground()?;
    Some(PreviousApp { name, handle })
}

/// Asks the OS to give focus back to `app`. Returns whether the request was
/// made; the window manager has the final say.
pub fn activate(app: &PreviousApp) -> bool {
    imp::activate(&app.handle)
}

/// One display's work area (menu bar and dock excluded), in that display's
/// own logical units, positioned in the OS's global screen space:
/// top-left physical pixels on Windows, AppKit points with a bottom-left
/// origin on macOS. These are exactly the units each platform's windowing
/// API accepts for `LogicalPosition`/`LogicalSize`, so a window can be
/// placed from them without any scale conversion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisplayArea {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Device pixels per point on this display (1 on standard, 2 on Retina).
    pub scale: f64,
    /// Whether `x`/`y`/`width`/`height` are physical pixels (true, Windows)
    /// rather than logical points (false, macOS).
    pub units_physical: bool,
}

/// The work area of the display the mouse pointer is currently on, so a
/// window summoned with a shortcut opens where the user is working.
///
/// `None` means the platform cannot tell (Wayland) or the pointer is off
/// every screen; the caller should fall back to its own monitor logic.
pub fn cursor_display_work_area() -> Option<DisplayArea> {
    imp::cursor_display_work_area()
}
