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
| Capture window focus + typing | verified | verified (Ubuntu 24.04.4, headless `sway 1.9`, WebKitGTK `2.52.6`, `wtype 0.4-3`: `--capture` focus, typing, and synthetic `Ctrl+V` / `Shift+Insert` paste; `niri` not run) | in daily use | not run |
| Window follows the pointer's display | verified (two mixed-DPI displays) | compositor decides | verified (two mixed-DPI displays) | not run |
| Focus returns to previous app | verified | left to compositor | in daily use | not run |
| Draft survives `kill -9` | verified | same code | same code | same code |
| Single instance, `--capture` | verified | verified (Ubuntu 24.04.4, headless `sway 1.9`, WebKitGTK `2.52.6`) | in daily use | not run |
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
- **Capture focus and virtual-keyboard paste**: the capture window re-focuses
  its textarea on window `focus` when nothing else in the window is focused,
  in case `capture-shown` arrived before the webview had keyboard focus. On
  Linux, `Ctrl+V` and `Shift+Insert` inside the capture textarea read the
  clipboard through GTK (`capture_clipboard_read`) and insert at the selection,
  because WebKitGTK ignores paste chords sent via `zwp_virtual_keyboard_v1`
  (`wtype`, whose virtual keymap maps keys starting at hardware keycode 9) and
  blocks `navigator.clipboard.readText()`. Verified on Ubuntu 24.04.4 under
  headless `sway 1.9` with WebKitGTK `2.52.6` and `wtype 0.4-3`; `niri` and
  `ydotool` were not run.

### Linux, general

- Rounded corners and the shadow need a compositing window manager. Without
  one the capture window gets square black corners.
- The tray icon needs a StatusNotifier host. Stock GNOME has none (install the
  AppIndicator extension). The app cannot detect a missing host; use the
  shortcut, or launch Posthaste again to bring the window back.
- Some window managers apply focus-stealing prevention. If the capture window
  appears without focus, that policy is the cause.

### macOS

- Capture, Today and Note details are non-activating `NSPanel`s (`tauri-nspanel`,
  `src-tauri/src/panels.rs`), so they appear over full-screen spaces and take
  the keyboard without pulling other Posthaste windows forward or switching
  desktops. When hidden, they stay warm at `alphaValue = 0.0` (`ignoresMouseEvents = true`)
  so `WKWebView` keeps its GPU backing store and opens in a single frame without
  a black flash.
- The system reports no conflict when another app already uses a shortcut;
  registration succeeds and whichever app registered first wins. Posthaste
  cannot detect this.
- Transparent windows use Tauri's `macOSPrivateApi`, which rules out the Mac
  App Store.
- Release builds are signed for the in-app updater; macOS Gatekeeper notarisation
  is not configured yet.

### Windows (unverified)

- A shortcut held by another application fails to register, and the app says
  so and opens Settings.
- Needs the WebView2 runtime (preinstalled on Windows 11).
- "Active display" means the display under the mouse pointer on every OS.

## How it is built

```
ui/                       Rust + Leptos UI compiled to WebAssembly (no business logic)
  src/ipc/                the only module that calls the native side; typed
  src/logic/              pure logic: dates, query building, shortcuts, steps, projects
  src/components/         Button, inputs, Switch, Segmented, Dialog, Toasts, Kbd, Scene, …
  src/windows/            capture, today, details, and main (inbox, trash, settings)
  styles/tokens.css       design tokens: colour, type, spacing, motion
  tests/                  browser tests against a fake Tauri shell (harness.mjs)
locales/                  interface text for every language (en/, vi/)
src-tauri/
  crates/core/            storage. SQLite, migrations, notes, steps, projects, drafts,
                          search, settings, looks, sync engine, export/import.
                          No Tauri or iroh dependency.
  crates/model/           shared serde types across the IPC boundary
  crates/i18n/            compile-time checked language catalogue and Translator
  crates/platform/        per-OS "which app was in front / give focus back"
  crates/sync-net/        iroh QUIC peer-to-peer transport and pairing codes
  src/                    Tauri shell: windows, panels, shortcuts, tray, sync, ai, commands
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
- **Search** is SQLite FTS5 (prefix matching, diacritics ignored) across notes,
  descriptions and steps, kept in sync by triggers. Input is tokenised before
  it reaches FTS, so no query syntax can break it.
- **Migrations** are versioned with `PRAGMA user_version`, each in its own
  transaction. Before upgrading a database that has data, a snapshot is written
  next to it (`posthaste.db.before-upgrade-from-vN.bak`). A database from a
  newer version is refused, not modified.
- **Delete** moves a note to Trash for 30 days (`TRASH_RETENTION_MS`), which is
  what makes Undo and restore possible.
- **rusqlite, not SQLx**: one local user and one connection behind a mutex; an
  async pool would add weight without benefit.
- **Logging** records events and error kinds, never note text.

Data location: `<app data dir>/dev.posthaste.desktop/posthaste.db`
(`~/.local/share/…` on Linux, `~/Library/Application Support/…` on macOS,
`%APPDATA%\…` on Windows). Development builds (`debug_assertions`) open
`posthaste-dev.db` in the same folder so local testing never upgrades or
modifies your installed app's notes. Settings → Data shows the exact path.

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

## Window position

Capture and Today each have their own position: one spot of a 3×3 grid on the
display under the pointer, 8 logical pixels from the edges it sits against
(the window's transparent shadow margin adds to that). Settings from before
0.3.2 had one shared position; Today starts where capture was, and the old
"Upper third" loads as Top center, which now sits near the top edge rather
than 22% down. The details window always opens centred.

## Window and text sizes

Capture, Today and the details window's first-open size are base sizes times
the Window size setting: Compact 0.9, Standard 1.0 (the original sizes), Roomy
1.2. Capture's width is still 42% of the display, within bounds that scale the
same way. Every panel is clamped to the display's work area in both width and
height, so Roomy stays on a short display. The details window gets its base
size on its first open in a run; after that it keeps the size the user left
it at, and changing the setting resizes it afresh on its next open.

The Text size setting (Small, Standard or Large) sets `data-text-size` on the
root `<html>` element in all four windows, scaling the typography tokens
(`--t-xs` through `--t-title`) in `ui/styles/tokens.css`.

Each time the details window opens from hidden it is centred on the display
the pointer is on, like capture and Today. On macOS all three floating windows
are non-activating `NSPanel`s with `can_join_all_spaces` and
`full_screen_auxiliary`, so they open on the desktop or full-screen Space the
user is currently on instead of switching Spaces.


## AI providers and keys

AI polish can use any provider in `src-tauri/src/ai/providers.rs`; adding
one that speaks the OpenAI chat format is a single entry there. Requests go
straight from this computer to the provider (or to the server the user runs).

Keys are stored with the `keyring` crate: macOS Keychain, Windows Credential
Manager, or the Secret Service on Linux (GNOME Keyring, KWallet). When no
keychain answers at startup — common on minimal Linux setups with no Secret
Service running — keys go to a `secret:ai:<provider>` row in the database
instead, apart from the settings the interface reads, and Settings says
which store is in use. Keys from 0.3.1 and earlier (plain text in the
settings) move to the keychain on the first launch of 0.3.2.

On macOS an unsigned development build may ask for keychain access again
after each rebuild, because its code signature changes.
The keychain is read only when a key is used (polish, Test) or saved; opening
Settings never reads it, so it never asks for a password. "Key saved" comes
from a marker in the database that holds no secret. Keys from an older build
are noticed once, at the first launch that has markers.

GitHub Models is not offered: GitHub retired the service on 30 July 2026,
and GitHub Copilot has no public API for other apps. A saved choice of
GitHub Models (from a development build) reverts to Off.

Checked on Linux without a keychain (database fallback, key moved from old
settings, Settings shown). Not yet checked on macOS or Windows.

## Floating windows are non-activating panels (macOS)

Showing an ordinary window makes its application active, and activating
Posthaste brings every Posthaste window in front of the app you are working
in, greys that app out and can switch desktops. So on macOS the capture,
Today and Note details windows are turned into `NSPanel`s with the
non-activating style (through `tauri-nspanel`, `src-tauri/src/panels.rs`),
like Spotlight: they take the keyboard (`make_key_and_order_front`) while
Posthaste stays in the background and the app you were in stays active behind
them. They float on every desktop and above apps in full screen, and when
dismissed they are kept warm at `alphaValue = 0.0` (`ignoresMouseEvents = true`)
rather than ordered out so `WKWebView` never purges its GPU backing store. If
a window cannot be made a panel it is shown the ordinary way.
