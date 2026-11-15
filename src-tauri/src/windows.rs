//! The three windows: the floating capture panel, the Today glance and the
//! main inbox.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use posthaste_core::CapturePosition;
use serde::Serialize;
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, PhysicalPosition, PhysicalSize,
    WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};

use crate::state::{lock, AppState};

pub const MAIN: &str = "main";
pub const CAPTURE: &str = "capture";
pub const TODAY: &str = "today";

/// Logical size of the capture window, including the transparent margin its
/// shadow is drawn in.
const CAPTURE_HEIGHT: f64 = 268.0;
const CAPTURE_MIN_WIDTH: f64 = 520.0;
const CAPTURE_MAX_WIDTH: f64 = 700.0;
/// Logical size of the Today window, including the transparent margin its
/// shadow is drawn in.
const TODAY_WIDTH: f64 = 440.0;
const TODAY_HEIGHT: f64 = 540.0;
/// Focus changes this soon after showing are the window manager settling,
/// not the user leaving.
const BLUR_GRACE: Duration = Duration::from_millis(300);

#[derive(Clone, Serialize)]
struct CaptureShown {
    /// Name of the app the user was in, when known.
    source: Option<String>,
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let main = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App("index.html".into()))
        .title("Posthaste")
        .inner_size(960.0, 660.0)
        .min_inner_size(620.0, 440.0)
        .center()
        .visible(false);
    #[cfg(target_os = "macos")]
    let main = main
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);
    let main = main.build()?;

    let handle = app.clone();
    main.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            // Closing the inbox must not stop the capture shortcut working.
            api.prevent_close();
            if let Some(w) = handle.get_webview_window(MAIN) {
                let tray = handle
                    .state::<AppState>()
                    .tray_available
                    .load(Ordering::Relaxed);
                // Without a tray there would be no way back to a hidden
                // window, so keep it reachable in the taskbar instead.
                let _ = if tray { w.hide() } else { w.minimize() };
            }
        }
    });

    // Created once, hidden, and reused: showing an existing window is what
    // makes the shortcut feel instant.
    let capture = WebviewWindowBuilder::new(app, CAPTURE, WebviewUrl::App("index.html".into()))
        .title("Posthaste")
        .inner_size(620.0, CAPTURE_HEIGHT)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .maximizable(false)
        .minimizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible_on_all_workspaces(true)
        .visible(false)
        .build()?;

    let handle = app.clone();
    capture.on_window_event(move |event| match event {
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            hide_capture(&handle, true);
        }
        WindowEvent::Focused(false) => {
            let state = handle.state::<AppState>();
            let settled = lock(&state.capture_shown_at)
                .map(|t| t.elapsed() > BLUR_GRACE)
                .unwrap_or(false);
            if settled && state.settings().hide_on_blur {
                // The user moved on by themselves: don't pull focus back.
                hide_capture(&handle, false);
            }
        }
        _ => {}
    });

    let today = WebviewWindowBuilder::new(app, TODAY, WebviewUrl::App("index.html".into()))
        .title("Posthaste Today")
        .inner_size(TODAY_WIDTH, TODAY_HEIGHT)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .maximizable(false)
        .minimizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible_on_all_workspaces(true)
        .visible(false)
        .build()?;

    let handle = app.clone();
    today.on_window_event(move |event| match event {
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            hide_today(&handle, true);
        }
        WindowEvent::Focused(false) => {
            let state = handle.state::<AppState>();
            let settled = lock(&state.today_shown_at)
                .map(|t| t.elapsed() > BLUR_GRACE)
                .unwrap_or(false);
            if settled && state.settings().hide_on_blur {
                // The user moved on by themselves: don't pull focus back.
                hide_today(&handle, false);
            }
        }
        _ => {}
    });
    Ok(())
}

pub fn toggle_capture(app: &AppHandle) {
    let Some(window) = app.get_webview_window(CAPTURE) else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        hide_capture(app, true);
    } else {
        show_capture(app);
    }
}

pub fn show_capture(app: &AppHandle) {
    let Some(window) = app.get_webview_window(CAPTURE) else {
        log::error!("capture window is missing");
        return;
    };
    let state = app.state::<AppState>();

    // Must happen before our window takes focus.
    let previous = posthaste_platform::foreground_app();
    let source = previous.as_ref().and_then(|p| p.name.clone());
    *lock(&state.previous_app) = previous;

    let placed = place_capture(app, &window, state.settings().capture_position);
    *lock(&state.capture_shown_at) = Some(Instant::now());

    if let Err(e) = window.show() {
        log::error!("capture window could not be shown: {e}");
        return;
    }
    // Some window managers only honour a position once the window is mapped.
    if let Some(pos) = placed {
        let _ = window.set_position(pos);
    }
    let _ = window.set_always_on_top(true);
    if let Err(e) = window.set_focus() {
        log::warn!("capture window could not take focus: {e}");
    }
    let _ = window.emit("capture-shown", CaptureShown { source });
}

/// Hides the capture window. With `restore_focus`, hands focus back to the
/// application the user was in when they triggered capture.
pub fn hide_capture(app: &AppHandle, restore_focus: bool) {
    let state = app.state::<AppState>();
    *lock(&state.capture_shown_at) = None;
    let previous = lock(&state.previous_app).take();
    let Some(window) = app.get_webview_window(CAPTURE) else {
        return;
    };
    let handle = app.clone();
    // One main-thread step, so the hide and the re-activation cannot reorder.
    let result = app.run_on_main_thread(move || {
        let _ = window.emit("capture-hidden", ());
        if let Err(e) = window.hide() {
            log::error!("capture window could not be hidden: {e}");
        }
        if !restore_focus {
            return;
        }
        match previous {
            Some(prev) if posthaste_platform::activate(&prev) => {}
            Some(_) => log::debug!("previous application could not be re-activated"),
            None => release_focus(&handle),
        }
    });
    if let Err(e) = result {
        log::error!("could not schedule hiding the capture window: {e}");
    }
}

pub fn toggle_today(app: &AppHandle) {
    let Some(window) = app.get_webview_window(TODAY) else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        hide_today(app, true);
    } else {
        show_today(app);
    }
}

pub fn show_today(app: &AppHandle) {
    let Some(window) = app.get_webview_window(TODAY) else {
        log::error!("today window is missing");
        return;
    };
    let state = app.state::<AppState>();

    // Must happen before our window takes focus.
    let previous = posthaste_platform::foreground_app();
    *lock(&state.today_previous_app) = previous;

    let placed = place_panel(
        app,
        &window,
        state.settings().capture_position,
        // A glance has one size: fixed width, clamped to the display.
        |_| TODAY_WIDTH,
        TODAY_HEIGHT,
    );
    *lock(&state.today_shown_at) = Some(Instant::now());

    if let Err(e) = window.show() {
        log::error!("today window could not be shown: {e}");
        return;
    }
    // Some window managers only honour a position once the window is mapped.
    if let Some(pos) = placed {
        let _ = window.set_position(pos);
    }
    let _ = window.set_always_on_top(true);
    if let Err(e) = window.set_focus() {
        log::warn!("today window could not take focus: {e}");
    }
    let _ = window.emit("today-shown", ());
}

/// Hides the Today window. With `restore_focus`, hands focus back to the
/// application the user was in when they opened the glance.
pub fn hide_today(app: &AppHandle, restore_focus: bool) {
    let state = app.state::<AppState>();
    *lock(&state.today_shown_at) = None;
    let previous = lock(&state.today_previous_app).take();
    let Some(window) = app.get_webview_window(TODAY) else {
        return;
    };
    let handle = app.clone();
    // One main-thread step, so the hide and the re-activation cannot reorder.
    let result = app.run_on_main_thread(move || {
        if let Err(e) = window.hide() {
            log::error!("today window could not be hidden: {e}");
        }
        if !restore_focus {
            return;
        }
        match previous {
            Some(prev) if posthaste_platform::activate(&prev) => {}
            Some(_) => log::debug!("previous application could not be re-activated"),
            None => release_focus(&handle),
        }
    });
    if let Err(e) = result {
        log::error!("could not schedule hiding the today window: {e}");
    }
}

/// Gives focus up when no previous application was recorded: Posthaste
/// itself was in front, or the platform cannot tell (Wayland). On macOS,
/// hiding the app returns focus to whatever is behind it; we only do that
/// when the inbox isn't open, since it would be hidden too. Elsewhere the
/// window manager picks the next window by itself.
#[cfg(target_os = "macos")]
fn release_focus(app: &AppHandle) {
    let inbox_open = app
        .get_webview_window(MAIN)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    if !inbox_open {
        let _ = app.hide();
    }
}

#[cfg(not(target_os = "macos"))]
fn release_focus(_app: &AppHandle) {}

/// Shows and focuses the inbox, optionally switching to a view
/// ("inbox", "today", "archive", "settings").
pub fn show_main(app: &AppHandle, view: Option<&str>) {
    let Some(window) = app.get_webview_window(MAIN) else {
        log::error!("main window is missing");
        return;
    };
    let _ = window.unminimize();
    if let Err(e) = window.show() {
        log::error!("main window could not be shown: {e}");
    }
    let _ = window.set_focus();
    if let Some(view) = view {
        let _ = window.emit("navigate", view);
    }
}

/// Sizes the capture window for the display the pointer is on and returns
/// where it should go.
fn place_capture(
    app: &AppHandle,
    window: &WebviewWindow,
    position: CapturePosition,
) -> Option<tauri::Position> {
    // Capture takes a slice of the display, within its width bounds.
    place_panel(
        app,
        window,
        position,
        |logical_area| (logical_area * 0.42).clamp(CAPTURE_MIN_WIDTH, CAPTURE_MAX_WIDTH),
        CAPTURE_HEIGHT,
    )
}

/// One display's work area in the OS's global screen space, in the units
/// that display's windowing API accepts for `LogicalPosition`/`LogicalSize`
/// (AppKit points on macOS, physical pixels on Windows) — exactly what
/// `posthaste_platform::cursor_display_work_area` reports.
struct Area {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    scale: f64,
    /// Whether x/y/w/h are physical pixels (Windows) rather than logical
    /// points (macOS).
    units_physical: bool,
}

/// Where a panel goes on an area: centred, sized from `width_for` (given
/// the area's logical width), clamped to the area and kept above a small
/// floor. Pure, so the geometry is testable without a real window. The
/// returned rectangle is in the area's own units: x/y for the panel's
/// top-left corner, w/h for its size.
fn compute_layout(
    area: &Area,
    position: CapturePosition,
    width_for: impl Fn(f64) -> f64,
    logical_height: f64,
) -> (f64, f64, f64, f64) {
    let width = width_for(area.w);
    let width = width.min(area.w - 16.0).max(280.0);
    let w = width.round();
    let h = logical_height.round();

    let x = area.x + (area.w - w) / 2.0;
    let y = match position {
        CapturePosition::Center => area.y + (area.h - h) / 2.0,
        CapturePosition::Upper => area.y + (area.h * 0.22).min(area.h - h),
    };
    (x.round(), y.round().max(area.y), w, h)
}

fn place_in_area(
    area: &Area,
    window: &WebviewWindow,
    position: CapturePosition,
    width_for: impl Fn(f64) -> f64,
    logical_height: f64,
) -> tauri::Position {
    let (x, y, w, h) = compute_layout(area, position, width_for, logical_height);
    // Logical units pass straight through the window APIs — no scale
    // conversion with the window's previous display, which is what put the
    // panel on the wrong screen across mixed-DPI displays. Windows reports
    // physical pixels, so those go in as physical instead.
    let (position, size): (tauri::Position, tauri::Size) = if area.units_physical {
        (
            PhysicalPosition::new(x as i32, y as i32).into(),
            PhysicalSize::new((w * area.scale) as u32, (h * area.scale) as u32).into(),
        )
    } else {
        (
            LogicalPosition::new(x, y).into(),
            LogicalSize::new(w, h).into(),
        )
    };
    // Position first so the size is applied at the destination display's scale.
    let _ = window.set_position(position);
    // The window is fixed-size by pinning min and max to the same value
    // rather than by `resizable(false)`, which on GTK would also stop us
    // resizing it for a different display.
    let _ = window.set_min_size(Some(size));
    let _ = window.set_max_size(Some(size));
    let _ = window.set_size(size);
    position
}

/// Sizes and places a floating panel on the display the pointer is on and
/// returns where it should go. `None` means no display could be determined
/// (typical on Wayland, where the compositor places windows itself).
fn place_panel(
    app: &AppHandle,
    window: &WebviewWindow,
    position: CapturePosition,
    width_for: impl Fn(f64) -> f64,
    logical_height: f64,
) -> Option<tauri::Position> {
    // Preferred: the display under the pointer, measured natively. Tauri's
    // own cursor_position mixes physical and logical units on macOS, which
    // put the window on the wrong screen on mixed-DPI setups.
    if let Some(area) = posthaste_platform::cursor_display_work_area() {
        return Some(place_in_area(
            &Area {
                x: area.x,
                y: area.y,
                w: area.width,
                h: area.height,
                scale: area.scale,
                units_physical: area.units_physical,
            },
            window,
            position,
            width_for,
            logical_height,
        ));
    }

    // Fallback: Tauri's monitor chain, normalised to logical units. Correct
    // on X11, where cursor and monitor rectangles already share one space.
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten())?;
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let area = Area {
        x: area.position.x as f64 / scale,
        y: area.position.y as f64 / scale,
        w: area.size.width as f64 / scale,
        h: area.size.height as f64 / scale,
        scale,
        units_physical: false,
    };
    Some(place_in_area(
        &area,
        window,
        position,
        width_for,
        logical_height,
    ))
}

#[cfg(test)]
mod tests {
    use super::{compute_layout, Area};
    use posthaste_core::CapturePosition;

    /// The laptop display: 1512x982 points at 2x, menu bar 25, dock 25.
    fn retina() -> Area {
        Area {
            x: 0.0,
            y: 25.0,
            w: 1512.0,
            h: 932.0,
            scale: 2.0,
            units_physical: false,
        }
    }

    /// A 1x display parked to the right.
    fn external() -> Area {
        Area {
            x: 1512.0,
            y: 0.0,
            w: 1920.0,
            h: 1048.0,
            scale: 1.0,
            units_physical: true,
        }
    }

    fn capture_width(logical: f64) -> f64 {
        (logical * 0.42).clamp(520.0, 700.0)
    }

    #[test]
    fn capture_centers_on_the_work_area() {
        let (x, y, w, h) = compute_layout(&retina(), CapturePosition::Center, capture_width, 268.0);
        assert_eq!((x, y, w, h), (439.0, 357.0, 635.0, 268.0)); // (1512-635)/2, 25+(932-268)/2
    }

    #[test]
    fn capture_sits_in_the_upper_third_when_asked() {
        let (_, y, _, _) = compute_layout(&retina(), CapturePosition::Upper, capture_width, 268.0);
        assert_eq!(y, 230.0); // 25 + 932 * 0.22, never below the panel
    }

    #[test]
    fn today_keeps_its_fixed_width_on_either_display() {
        let width = |_| 440.0;
        let (x, y, w, h) = compute_layout(&retina(), CapturePosition::Center, width, 540.0);
        assert_eq!((x, y, w, h), (536.0, 221.0, 440.0, 540.0)); // (1512-440)/2, 25+(932-540)/2

        let (x, _, w, h) = compute_layout(&external(), CapturePosition::Center, width, 540.0);
        assert_eq!((x, w, h), (2252.0, 440.0, 540.0)); // 1512 + (1920-440)/2
    }

    #[test]
    fn narrow_displays_clamp_the_panel_to_the_screen() {
        let tiny = Area {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 600.0,
            scale: 1.0,
            units_physical: true,
        };
        let (x, _, w, _) = compute_layout(&tiny, CapturePosition::Center, capture_width, 268.0);
        // 0.42 * 800 = 336 < 520 floor; then clamped below 800-16 so it fits.
        assert_eq!(w, 520.0);
        assert_eq!(x, 140.0);
    }
}
