use objc2::rc::Retained;
use objc2::{msg_send, ClassType};
use objc2_app_kit::{
    NSApplicationActivationOptions, NSEvent, NSRunningApplication, NSScreen, NSWorkspace,
};
use objc2_foundation::NSArray;

use crate::{DisplayArea, Session};

/// Process id of the previously frontmost application.
pub type Handle = i32;

pub fn session() -> Session {
    Session::MacOs
}

pub fn foreground() -> Option<(Handle, Option<String>)> {
    let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
    let pid = app.processIdentifier();
    if pid == std::process::id() as i32 {
        return None;
    }
    let name = app.localizedName().map(|n| n.to_string());
    Some((pid, name))
}

pub fn activate(pid: &Handle) -> bool {
    match NSRunningApplication::runningApplicationWithProcessIdentifier(*pid) {
        Some(app) => app.activateWithOptions(NSApplicationActivationOptions(0)),
        None => false,
    }
}

/// The work area of the screen under the mouse pointer.
///
/// Hit-testing happens in AppKit's own coordinate space — `NSEvent::mouseLocation`
/// and every `NSScreen::frame` are both in points with the primary screen's
/// bottom-left corner as the global origin — so no unit or axis conversion can
/// go wrong between mixed-DPI displays (Retina MacBook + 1x external). The
/// result is converted to Tauri's top-left physical space the same way tao
/// converts monitors: per-screen scale, primary at `(0, 0)`.
pub fn cursor_display_work_area() -> Option<DisplayArea> {
    // The class method +[NSScreen screens] is read-only and is queried from
    // whatever thread we happen to be on, the same way tao's own monitor
    // code does it: global-shortcut, tray and single-instance callbacks all
    // run off the main thread here.
    //
    // SAFETY: calling a declared AppKit class method; the returned array is
    // retained per Obj-C memory rules and wrapped immediately.
    let screens: Retained<NSArray<NSScreen>> = unsafe { msg_send![NSScreen::class(), screens] };
    let mouse = NSEvent::mouseLocation();

    let mut target = None;
    for screen in screens.iter() {
        let frame = screen.frame();
        if mouse.x >= frame.origin.x
            && mouse.x < frame.origin.x + frame.size.width
            && mouse.y >= frame.origin.y
            && mouse.y < frame.origin.y + frame.size.height
        {
            target = Some(screen);
            break;
        }
    }
    let screen = target?;

    // visibleFrame verbatim: AppKit points, bottom-left origin — the exact
    // units a LogicalPosition/LogicalSize is used in by the window APIs.
    let visible = screen.visibleFrame();
    Some(DisplayArea {
        x: visible.origin.x,
        y: visible.origin.y,
        width: visible.size.width,
        height: visible.size.height,
        scale: screen.backingScaleFactor(),
        units_physical: false,
    })
}
