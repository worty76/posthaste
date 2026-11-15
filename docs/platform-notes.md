# Platform behaviour and limitations

Kept out of the README so it stays short. Everything here marked **verified**
was actually run on the named platform; anything else is an honest "not run"
or a read-only review of the code, never a claim.

### What was verified, and where

The original development ran on Ubuntu 24.04, X11 (Xvfb + openbox + picom),
WebKitGTK 2.52. Since then the app is built, installed and used daily on
macOS (Apple Silicon).

| | Linux X11 | Linux Wayland | macOS | Windows |
|---|---|---|---|---|
| Builds | verified | same binary | verified (Apple Silicon) | **not built** |
| Global shortcut | verified | limited, see below | not verified | not run |
| Capture window focus + typing | verified | not run | in daily use | not run |
| Window follows the pointer's display | verified (two mixed-DPI displays) | compositor decides | verified (two mixed-DPI displays) | not run |
| Focus returns to previous app | verified | left to compositor | in daily use | not run |
| Draft survives `kill -9` | verified | same code | same code | same code |
| Single instance, `--capture` | verified | not run | in daily use | not run |
| Tray menu | code runs; no tray host in the test display | not run | not verified | not run |
| Today window | not run (the e2e script covers it; run it on Linux) | not run | opens with `--today` and renders; keys not exercised | not run |
| Note details window | e2e (Xvfb + openbox): Enter in Today opens it focused, typing autosaves, Esc saves, hides it and returns to Today, focus ends in the previous app; the window and attachment tiles checked by screenshot | not run | not run | not run |
| Later (L in Today) | e2e (Xvfb + openbox): L parks the selected note, Today stays open and focused | not run | not run | not run |
| Day view and origin tag in Today | e2e (Xvfb + openbox): X does nothing on a past day, T returns to today, Esc still closes | not run | not run | not run |
| Calendar in the inbox | unit tests only | not run | not run | not run |
| Trash (restore, delete forever) | e2e (Xvfb + openbox): Delete, then R in Trash restores; Delete forever erases only that note | not run | not run | not run |
| Window size setting | e2e (Xvfb + openbox): Roomy opens capture and Today 1.2 times the size | not run | not run | not run |
| Launch at login | not exercised | not exercised | not verified | not run |
| Multiple monitors, HiDPI | **not tested** | not tested | not tested | not tested |

The macOS and Windows code in `crates/platform` was checked by reading it
against the `objc2-app-kit` and `windows-sys` sources; macOS compiles and
runs (Apple Silicon). Expect to fix small things on the first Windows build.
The `#[cfg(target_os = "macos")]` blocks in `src-tauri/src` (overlay title
bar, Dock reopen, template tray icon) are in the same state.

### Linux, Wayland

Wayland does not let an application register global shortcuts, read which
window is focused, or position its own windows.

- **Shortcut**: registration goes through XWayland and may only fire while an
  X11 window has focus. Settings says so instead of showing "active".
  Reliable route: bind `posthaste --capture` to a key in your desktop's
  keyboard settings (GNOME: Settings → Keyboard → Custom Shortcuts; KDE:
  Shortcuts → Add Command). It reaches the running instance in a few
  milliseconds. Support for the `GlobalShortcuts` desktop portal would remove
  this step and is not implemented.
- **Position**: the compositor places the window, usually centred on the
  active output. The "Window position" setting may have no effect.
- **Focus return**: Posthaste cannot ask for it, but compositors return focus
  to the previous window when ours hides.
- **"from <app>"** on notes is not available.

### Linux, general

- Rounded corners and the shadow need a compositing window manager. Without
  one the capture window gets square black corners.
- The tray icon needs a StatusNotifier host. Stock GNOME has none (install the
  AppIndicator extension). The app cannot detect a missing host; use the
  shortcut, or launch Posthaste again to bring the window back.
- Some window managers apply focus-stealing prevention. If the capture window
  appears without focus, that policy is the cause.

### macOS

- The capture window is a normal always-on-top window, not an `NSPanel`. It
  will not appear over another application's **full-screen** space. Fixing
  that needs a non-activating panel.
- The system reports no conflict when another app already uses a shortcut;
  registration succeeds and whichever app registered first wins. Posthaste
  cannot detect this.
- Transparent windows use Tauri's `macOSPrivateApi`, which rules out the Mac
  App Store.
- The app is not signed or notarised.

### Windows (unverified)

- A shortcut held by another application fails to register, and the app says
  so and opens Settings.
- Needs the WebView2 runtime (preinstalled on Windows 11).
- "Active display" means the display under the mouse pointer on every OS.

## How it is built

```
src/                      React + TypeScript UI (no business logic)
  lib/ipc.ts              the only module that calls the native side; typed
  lib/                    pure logic: dates, query building, shortcuts, draft sync
  components/             Button, inputs, Switch, Segmented, Dialog, Toasts, Kbd, …
  capture/                the floating capture window
  main/                   inbox, search, settings
  styles/tokens.css       design tokens: colour, type, spacing, motion
src-tauri/
  crates/core/            storage. SQLite, migrations, notes, drafts, search,
                          settings, export/import. No Tauri dependency.
  crates/platform/        per-OS "which app was in front / give focus back"
  src/                    Tauri shell: windows, shortcuts, tray, commands
scripts/e2e-linux.sh      end-to-end test against real windows under Xvfb
```

Decisions worth knowing:

- **All four windows are created once at startup and hidden.** A shortcut
  only shows an existing window, which is why it appears quickly.
- **A note is "saved" only after SQLite commits** (`journal_mode=WAL`,
  `synchronous=FULL`). The capture window clears and hides after that, never
  before. On failure the text stays and the error is shown.
- **Drafts** are written to the `drafts` table ~250 ms after each keystroke and
  flushed on Esc, focus loss and hide. Saving a note and clearing its draft is
  one transaction.
- **Search** is SQLite FTS5 (prefix matching, diacritics ignored), kept in sync
  by triggers. Input is tokenised before it reaches FTS, so no query syntax can
  break it.
- **Migrations** are versioned with `PRAGMA user_version`, each in its own
  transaction. Before upgrading a database that has data, a snapshot is written
  next to it (`posthaste.db.before-upgrade-from-vN.bak`). A database from a
  newer version is refused, not modified.
- **Delete** moves a note to a trash that is emptied after 24 hours, which is
  what makes Undo possible.
- **rusqlite, not SQLx**: one local user and one connection behind a mutex; an
  async pool would add weight without benefit.
- **Logging** records events and error kinds, never note text.

Data location: `<app data dir>/dev.posthaste.desktop/posthaste.db`
(`~/.local/share/…` on Linux, `~/Library/Application Support/…` on macOS,
`%APPDATA%\…` on Windows). Settings → Data shows the exact path.

## Trash

A deleted note keeps its text and files in the database for 30 days
(`TRASH_RETENTION_MS` in `crates/core`), then the launch purge erases it.
Notes past 30 days are hidden from Trash and cannot be restored even before
that purge runs. "Delete forever" and "Empty Trash" erase at once. Until then
the text is still on disk, which matters if the computer is shared.

## Earlier days and the calendar

A past day in Today is rebuilt from timestamps, as it stood at the end of that
day. Only the latest completion, archive and Later times are kept, so a note
that was reopened or brought back from Later since then loses its earlier
history and shows as it is now. The calendar's first weekday comes from the
locale where the webview exposes it (`Intl.Locale` week info) and is Monday
otherwise; not checked on the macOS or Windows webviews.

## Window sizes

Capture, Today and the details window's first-open size are base sizes times
the Window size setting: Compact 0.9, Standard 1.0 (the original sizes), Roomy
1.2. Capture's width is still 42% of the display, within bounds that scale the
same way. Every panel is clamped to the display's work area in both width and
height, so Roomy stays on a short display. The details window is placed once
per run; changing the setting places it afresh on its next open.

