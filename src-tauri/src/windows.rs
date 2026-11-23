//! The four windows: the floating capture panel, the Today glance, the Note
//! details window and the main inbox.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use posthaste_core::{CapturePosition, CaptureShown, WindowSize};
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, PhysicalPosition, PhysicalSize,
    WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};

use crate::panels;
use crate::state::{lock, AppState};

pub const MAIN: &str = "main";
/// The one page every window loads. It starts the interface, which is
/// written in Rust (`ui/`); the window's label decides what it shows.
const UI: &str = "index.html";

pub const CAPTURE: &str = "capture";
pub const TODAY: &str = "today";
pub const DETAILS: &str = "details";

/// Logical size of the capture window, including the transparent margin its
/// shadow is drawn in.
const CAPTURE_HEIGHT: f64 = 268.0;
const CAPTURE_MIN_WIDTH: f64 = 520.0;
const CAPTURE_MAX_WIDTH: f64 = 700.0;
/// Logical size of the Today window, including the transparent margin its
/// shadow is drawn in.
const TODAY_WIDTH: f64 = 440.0;
const TODAY_HEIGHT: f64 = 540.0;
/// Logical size of the details window the first time it opens, and the
/// smallest it can be made.
const DETAILS_WIDTH: f64 = 680.0;
const DETAILS_HEIGHT: f64 = 620.0;
const DETAILS_MIN_WIDTH: f64 = 480.0;
const DETAILS_MIN_HEIGHT: f64 = 420.0;
/// Space between a floating window and the display edge it sits against.
/// The window's own transparent margin adds to it.
const EDGE_GAP: f64 = 8.0;
/// How long to wait before asking for focus again after hiding another of
/// our windows.
const REFOCUS_DELAY: Duration = Duration::from_millis(120);
/// Focus changes this soon after showing are the window manager settling,
/// not the user leaving.
const BLUR_GRACE: Duration = Duration::from_millis(300);

pub use posthaste_core::{DetailsFrom, DetailsTarget};

/// The target for a note opened from the window named `from`. Anything
/// that is not Today goes back to the inbox.
pub fn details_target(id: String, from: &str) -> DetailsTarget {
    let from = if from == "today" {
        DetailsFrom::Today
    } else {
        DetailsFrom::Inbox
    };
    DetailsTarget { id, from }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let main = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App(UI.into()))
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
    let capture = WebviewWindowBuilder::new(app, CAPTURE, WebviewUrl::App(UI.into()))
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
    panels::adopt(app, &capture);

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

    let today = WebviewWindowBuilder::new(app, TODAY, WebviewUrl::App(UI.into()))
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
    panels::adopt(app, &today);

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

    // Not always on top and not hidden on blur: picking a file or switching
    // to another app to copy something must not close it.
    let details = WebviewWindowBuilder::new(app, DETAILS, WebviewUrl::App(UI.into()))
        .title("Posthaste Note")
        .inner_size(DETAILS_WIDTH, DETAILS_HEIGHT)
        .min_inner_size(DETAILS_MIN_WIDTH, DETAILS_MIN_HEIGHT)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .maximizable(false)
        .minimizable(false)
        .skip_taskbar(true)
        .visible(false)
        .build()?;
    panels::adopt(app, &details);
    let handle = app.clone();
    details.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            // The page saves first, then asks to be dismissed.
            let _ = handle.emit_to(DETAILS, "details-close-requested", ());
        }
    });
    Ok(())
}

pub fn toggle_capture(app: &AppHandle) {
    let Some(window) = app.get_webview_window(CAPTURE) else {
        return;
    };
    if panels::is_visible(app, &window) {
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

    let settings = state.settings();
    let capture_position = settings.capture_position;
    let window_size = settings.window_size;
    *lock(&state.capture_shown_at) = Some(Instant::now());

    // Emit before ordering front so the webview starts focusing and updating
    // immediately rather than waiting for the panel show round-trip.
    let _ = window.emit("capture-shown", CaptureShown { source });

    // Place and show in one main-thread step so shortcut threads do not pay
    // multiple sequential dispatch_sync hops before the window appears.
    let handle = app.clone();
    let result = app.run_on_main_thread(move || {
        let placed = place_capture(&handle, &window, capture_position, window_size);
        let has_keyboard = match panels::show_on_main_thread(&handle, &window) {
            Ok(has_keyboard) => has_keyboard,
            Err(e) => {
                log::error!("capture window could not be shown: {e}");
                return;
            }
        };
        // Some Linux/Windows window managers only honour a position once the window
        // is mapped. On macOS, NSPanel already honours placement before showing,
        // and re-applying position/level after showing forces a second composite.
        #[cfg(not(target_os = "macos"))]
        {
            if let Some(pos) = placed {
                let _ = window.set_position(pos);
            }
            let _ = window.set_always_on_top(true);
        }
        #[cfg(target_os = "macos")]
        let _ = placed;
        if !has_keyboard {
            if let Err(e) = window.set_focus() {
                log::warn!("capture window could not take focus: {e}");
            }
        }
    });
    if let Err(e) = result {
        log::error!("could not schedule showing the capture window: {e}");
    }
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
        if let Err(e) = panels::hide_on_main_thread(&handle, &window) {
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
    if panels::is_visible(app, &window) {
        hide_today(app, true);
    } else {
        show_today(app);
    }
}

pub fn show_today(app: &AppHandle) {
    show_today_with(app, true);
}

/// Shows Today again after the details window closes: the app to hand focus
/// back to is still the one recorded when Today first opened, and the list
/// keeps its selection.
fn return_to_today(app: &AppHandle) {
    show_today_with(app, false);
}

fn show_today_with(app: &AppHandle, fresh: bool) {
    let Some(window) = app.get_webview_window(TODAY) else {
        log::error!("today window is missing");
        return;
    };
    let state = app.state::<AppState>();

    if fresh {
        // Must happen before our window takes focus.
        let previous = posthaste_platform::foreground_app();
        *lock(&state.today_previous_app) = previous;
    }

    let settings = state.settings();
    let today_position = settings.today_position;
    let window_size = settings.window_size;
    *lock(&state.today_shown_at) = Some(Instant::now());

    // Emit before ordering front so Today starts loading its list immediately.
    let _ = window.emit(
        if fresh {
            "today-shown"
        } else {
            "today-returned"
        },
        (),
    );

    // Place and show in one main-thread step to avoid sequential thread hops.
    let handle = app.clone();
    let result = app.run_on_main_thread(move || {
        let placed = place_panel(
            &handle,
            &window,
            today_position,
            // A glance has one size: fixed width, clamped to the display.
            |_| scaled(TODAY_WIDTH, window_size),
            scaled(TODAY_HEIGHT, window_size),
        );
        let has_keyboard = match panels::show_on_main_thread(&handle, &window) {
            Ok(has_keyboard) => has_keyboard,
            Err(e) => {
                log::error!("today window could not be shown: {e}");
                return;
            }
        };
        // Some Linux/Windows window managers only honour a position once the window
        // is mapped. On macOS, NSPanel already honours placement before showing,
        // and re-applying position/level after showing forces a second composite.
        #[cfg(not(target_os = "macos"))]
        {
            if let Some(pos) = placed {
                let _ = window.set_position(pos);
            }
            let _ = window.set_always_on_top(true);
        }
        #[cfg(target_os = "macos")]
        let _ = placed;
        if !has_keyboard {
            if let Err(e) = window.set_focus() {
                log::warn!("today window could not take focus: {e}");
            }
        }
    });
    if let Err(e) = result {
        log::error!("could not schedule showing the today window: {e}");
    }
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
        if let Err(e) = panels::hide_on_main_thread(&handle, &window) {
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

/// Opens the details window on a note. From Today, the glance steps aside
/// (keeping the app to return focus to) and comes back on Esc.
pub fn show_details(app: &AppHandle, target: DetailsTarget) {
    let Some(window) = app.get_webview_window(DETAILS) else {
        log::error!("details window is missing");
        return;
    };
    let state = app.state::<AppState>();
    let from_today = target.from == DetailsFrom::Today;
    if from_today {
        // Clearing the shown-at time first stops Today's blur handler from
        // hiding it and dropping the recorded app when details takes focus.
        *lock(&state.today_shown_at) = None;
    }
    *lock(&state.details_target) = Some(target.clone());

    let first = !state.details_placed.swap(true, Ordering::Relaxed);
    let placed = if first {
        let size = state.settings().window_size;
        let placed = place_panel(
            app,
            &window,
            CapturePosition::Center,
            |_| scaled(DETAILS_WIDTH, size),
            scaled(DETAILS_HEIGHT, size),
        );
        unpin_details_size(&window);
        placed
    } else {
        // Reopening: keep the size the user left it at, on the display the
        // pointer is on, as capture and Today do.
        let kept = window
            .inner_size()
            .ok()
            .zip(window.scale_factor().ok())
            .map(|(s, f)| s.to_logical::<f64>(f));
        let placed = kept.and_then(|s| {
            place_panel(app, &window, CapturePosition::Center, |_| s.width, s.height)
        });
        unpin_details_size(&window);
        placed
    };

    // As a panel it takes the keyboard without activating Posthaste, the
    // same way the capture and Today panels do: whatever app the user is
    // working in stays in front.
    let has_keyboard = match panels::show(app, &window) {
        Ok(has_keyboard) => has_keyboard,
        Err(e) => {
            log::error!("details window could not be shown: {e}");
            return;
        }
    };
    if let Some(pos) = placed {
        let _ = window.set_position(pos);
    }
    if !has_keyboard {
        if let Err(e) = window.set_focus() {
            log::warn!("details window could not take focus: {e}");
        }
    }
    let _ = window.emit("details-open", target);
    if from_today {
        // Today steps aside once details is up. Some window managers answer
        // a hidden window by focusing whatever was behind it, after our own
        // focus request, so details asks for focus once more afterwards.
        if let Some(today) = app.get_webview_window(TODAY) {
            let _ = panels::hide(app, &today);
        }
        refocus_later(app.clone(), window);
    }
}

/// place_panel pins the size; the details window is resizable.
fn unpin_details_size(window: &WebviewWindow) {
    let _ = window.set_max_size(None::<tauri::Size>);
    let _ = window.set_min_size(Some(tauri::Size::from(LogicalSize::new(
        DETAILS_MIN_WIDTH,
        DETAILS_MIN_HEIGHT,
    ))));
}

/// Asks for focus again shortly after a window-manager focus hand-off.
/// Panels re-take the keyboard through their own show path: tao's set_focus
/// runs `activateIgnoringOtherApps`, which would pull Posthaste in front of
/// the app the user is working in.
fn refocus_later(app: AppHandle, window: WebviewWindow) {
    std::thread::spawn(move || {
        std::thread::sleep(REFOCUS_DELAY);
        if panels::is_visible(&app, &window) {
            let has_keyboard = panels::show(&app, &window).unwrap_or_default();
            if !has_keyboard {
                // A regular window, or panels are unavailable: plain focus,
                // which activates the app — right for the inbox.
                let _ = window.set_focus();
            }
        }
    });
}

/// Hides the details window and goes back to where it was opened from.
pub fn hide_details(app: &AppHandle) {
    let state = app.state::<AppState>();
    let from = lock(&state.details_target).take().map(|t| t.from);
    // Show where we go back to first, then hide: the other order lets the
    // window manager focus an unrelated window in between.
    match from {
        Some(DetailsFrom::Today) => return_to_today(app),
        Some(DetailsFrom::Inbox) => show_main(app, None),
        None => {}
    }
    if let Some(window) = app.get_webview_window(DETAILS) {
        if let Err(e) = panels::hide(app, &window) {
            log::error!("details window could not be hidden: {e}");
        }
    }
    let back = match from {
        Some(DetailsFrom::Today) => app.get_webview_window(TODAY),
        Some(DetailsFrom::Inbox) => app.get_webview_window(MAIN),
        None => None,
    };
    if let Some(back) = back {
        refocus_later(app.clone(), back);
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
    // Refresh in case notes changed while the main window was hidden.
    let _ = window.emit("notes-changed", ());
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
    size: WindowSize,
) -> Option<tauri::Position> {
    // Capture takes a slice of the display, within its width bounds.
    place_panel(
        app,
        window,
        position,
        |logical_area| capture_width(logical_area, size),
        scaled(CAPTURE_HEIGHT, size),
    )
}

/// A base size grown or shrunk by the window size setting.
fn scaled(base: f64, size: WindowSize) -> f64 {
    (base * size.factor()).round()
}

/// Capture's width on a display `logical_area` wide: a slice of it, within
/// bounds that follow the window size setting.
fn capture_width(logical_area: f64, size: WindowSize) -> f64 {
    (logical_area * 0.42).clamp(
        scaled(CAPTURE_MIN_WIDTH, size),
        scaled(CAPTURE_MAX_WIDTH, size),
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
    // A tall window on a short display stays on it.
    let h = logical_height.min(area.h - 16.0).max(200.0).round();

    // One spot of a 3×3 grid: against an edge with a small gap, or centred.
    let gap = if area.units_physical {
        EDGE_GAP * area.scale
    } else {
        EDGE_GAP
    };
    let along = |start: f64, room: f64, size: f64, step: i8| match step {
        -1 => start + gap,
        1 => start + room - size - gap,
        _ => start + (room - size) / 2.0,
    };
    let (column, row) = position.grid();
    let x = along(area.x, area.w, w, column);
    let y = along(area.y, area.h, h, row);
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
    // On macOS, panels are hidden at alpha 0 while placed, so moving never
    // flashes; skip when the exact target coordinates already match the last
    // placement to avoid redundant AppKit frame notifications.
    let target_key = (x.round() as i64, y.round() as i64);
    let moved = {
        use std::collections::HashMap;
        use std::sync::{Mutex, OnceLock};
        static LAST_POS: OnceLock<Mutex<HashMap<String, (i64, i64)>>> = OnceLock::new();
        let mut map = LAST_POS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let prev = map.insert(window.label().to_owned(), target_key);
        prev != Some(target_key)
    };
    if moved {
        let _ = window.set_position(position);
    }
    // Skip resizing when the window is already at the target size: redundant
    // set_size calls on a transparent WKWebView invalidate its backing store
    // and flash a dark frame around the shadow margin.
    let current = window.inner_size().ok();
    let target_physical = match size {
        tauri::Size::Physical(s) => s,
        tauri::Size::Logical(s) => {
            let scale = window.scale_factor().unwrap_or(area.scale);
            PhysicalSize::new(
                (s.width * scale).round() as u32,
                (s.height * scale).round() as u32,
            )
        }
    };
    if current != Some(target_physical) {
        // The window is fixed-size by pinning min and max to the same value
        // rather than by `resizable(false)`, which on GTK would also stop us
        // resizing it for a different display.
        let _ = window.set_min_size(Some(size));
        let _ = window.set_max_size(Some(size));
        let _ = window.set_size(size);
    }
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
    use super::{capture_width as capture_width_for, compute_layout, scaled, Area};
    use posthaste_core::{CapturePosition, WindowSize};

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
    fn each_grid_spot_hugs_its_edges_with_the_same_gap() {
        // 440x540 on the retina work area (x 0..1512, y 25..957); gap 8.
        let at = |p| {
            let (x, y, _, _) = compute_layout(&retina(), p, |_| 440.0, 540.0);
            (x, y)
        };
        assert_eq!(at(CapturePosition::TopLeft), (8.0, 33.0));
        assert_eq!(at(CapturePosition::Top), (536.0, 33.0));
        assert_eq!(at(CapturePosition::TopRight), (1064.0, 33.0)); // 1512-440-8
        assert_eq!(at(CapturePosition::Left), (8.0, 221.0));
        assert_eq!(at(CapturePosition::Center), (536.0, 221.0));
        assert_eq!(at(CapturePosition::Right), (1064.0, 221.0));
        assert_eq!(at(CapturePosition::BottomLeft), (8.0, 409.0)); // 25+932-540-8
        assert_eq!(at(CapturePosition::Bottom), (536.0, 409.0));
        assert_eq!(at(CapturePosition::BottomRight), (1064.0, 409.0));
    }

    #[test]
    fn corners_stay_on_a_display_that_is_not_at_the_origin() {
        let (x, y, w, _) =
            compute_layout(&external(), CapturePosition::BottomRight, |_| 440.0, 540.0);
        assert_eq!((x, y, w), (2984.0, 500.0, 440.0)); // 1512+1920-440-8, 1048-540-8
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
    fn scaled_sizes_follow_the_preset() {
        assert_eq!(scaled(540.0, WindowSize::Medium), 540.0);
        assert_eq!(scaled(540.0, WindowSize::Small), 486.0);
        assert_eq!(scaled(540.0, WindowSize::Large), 648.0);
    }

    #[test]
    fn capture_width_bounds_scale_with_the_preset() {
        // Standard keeps the original bounds exactly.
        assert_eq!(capture_width_for(1512.0, WindowSize::Medium), 1512.0 * 0.42);
        assert_eq!(capture_width_for(1000.0, WindowSize::Medium), 520.0);
        assert_eq!(capture_width_for(3000.0, WindowSize::Medium), 700.0);
        assert_eq!(capture_width_for(3000.0, WindowSize::Large), 840.0);
        assert_eq!(capture_width_for(1000.0, WindowSize::Small), 468.0);
    }

    #[test]
    fn compute_layout_clamps_height_to_a_short_display() {
        let short = Area {
            x: 0.0,
            y: 0.0,
            w: 1280.0,
            h: 600.0,
            scale: 1.0,
            units_physical: false,
        };
        let (_, y, _, h) = compute_layout(&short, CapturePosition::Center, |_| 528.0, 648.0);
        assert_eq!(h, 584.0);
        assert_eq!(y, 8.0);
        // Every spot stays on the display too.
        for p in [CapturePosition::Top, CapturePosition::BottomRight] {
            let (_, y, _, h) = compute_layout(&short, p, |_| 528.0, 648.0);
            assert!(y >= 0.0 && y + h <= 600.0, "{p:?}");
        }
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
