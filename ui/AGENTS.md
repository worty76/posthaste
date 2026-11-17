# ui/: the Rust interface

Every window of Posthaste, written in Rust with Leptos and compiled to
WebAssembly. This code renders state and handles input. It decides nothing
about what a valid note is or how it is stored; `crates/core` owns that.

## Structure

```
src/lib.rs        entry point; the module table at its top is the map
src/logic/        decisions as plain functions. No browser, no Leptos
src/ipc/          every command and event. The only code that touches Tauri
src/time.rs       the clock, the time zone, date formatting
src/browser.rs    the clipboard, focus, text fields: small page helpers
src/state/        settings, language, look, updates: shared by windows
src/components/   controls that know nothing about notes
src/windows/      one module per window; main/ holds inbox, trash, settings
src/draft_sync.rs the autosave queue
styles/           tokens and stylesheets (see styles/AGENTS.md)
boot.js           loads the stylesheets and starts the module
tests/            browser tests against a fake shell
pkg/              build output (ignored by git)
```

Dependencies point one way: `windows` → `state`, `components`, `ipc`,
`logic`. `logic` imports none of the others. `components` never imports
`ipc`; a control that needs a command belongs in `windows`.

## Where code goes

- Dates: `logic/dates.rs` works in calendar days (`Day`) and knows no time
  zone. Only `time.rs` reads the clock, turns an instant into a local day,
  or formats a date. Always pass the language's locale; never format with
  the browser default.
- Can it be decided from plain values (a key press and the settings, an
  accelerator string, a theme)? It goes in `logic/` as a function with unit
  tests, and the window calls it. `logic/capture.rs` is the model.
- A window keeps its signals in one `Copy` struct and its actions as methods
  on it (`Capture` in `windows/capture.rs`). Handlers call those methods; the
  `view!` holds markup and wiring only.
- A new window: one module in `windows/`, one arm in `windows::mount`, its
  and its URL changed to `RUST_UI` in
  `src-tauri/src/windows.rs`. Delete the React window and its test in the
  same change, after the browser tests cover what that test covered.

## The IPC boundary

- `ipc/tauri.rs` holds the only bindings to `window.__TAURI__`. Nothing
  outside `ipc/` names a command or event as a string.
- `ipc/api.rs` has one typed function per command, next to its argument
  struct. `ipc/events.rs` has one marker type per event.
- Types come from `posthaste-model`, the crate storage and the shell use too.
  Do not redeclare a shape that exists there. A shape only this side reads
  (`AppInfo`, `ActiveLook`) may be a small view struct in `api.rs`.
- Failures are `CommandError { code, message }`. Show `ipc::message(..)` to
  the user; branch on `code`, never on message text.
- Do not decode an answer the window does not use (see `capture_save`): a
  shape change must never turn a committed save into a visible failure.

## Text and languages

- No user-visible text in Rust source. Every string is a key in
  `locales/en/*.json`, read through `I18n` (`state/i18n.rs`).
- Status and error lines store keys, not finished text, so they follow a
  language change (`Status` in `windows/capture.rs`).
- Whole sentences only: use `{placeholders}`, never glue fragments together.
- A new language is a folder under `locales/`; see `crates/i18n/AGENTS.md`.

## State

- Persistent state lives in SQLite through commands. No `localStorage`,
  `sessionStorage` or `indexedDB`.
- No state library and no global signals. Signals live in the window's
  struct and are passed down as props.
- Data changed elsewhere arrives as an event. Refetch on it; do not patch
  local copies to mirror what the database probably did.
- `state/appearance.rs` is the only writer of CSS custom properties and of
  `data-theme`, `data-shape`, `data-font`. `state/i18n.rs` is the only
  writer of `lang` and `dir`.
- To make an effect depend on a value it does not use, read it with
  `value.with(|_| ())`, never `value.track()`. A `Memo` that is only
  tracked is never computed, so it never subscribes to what it is made
  from and the effect never runs again. `scripts/check-rules.sh` enforces
  this.

## Keyboard, accessibility, styling

- Every action reachable by mouse is reachable by keyboard, and the reverse.
- Nothing fires while `event.is_composing()` is true. Enter during IME
  composition (Vietnamese Telex, CJK) must not save.
- A text field shows a signal through `use:bind_text`, never `prop:value`:
  writing back what was just typed ends the composition.
- Use `has_mod` for "⌘ on macOS, Ctrl elsewhere". Never test the two keys
  directly for a shortcut.
- Single-letter shortcuts never fire while focus is in a field. What a key
  does is decided by a `key_action` function in `logic/`, with tests.
- A new shortcut is added in three places together: `key_action`, the
  tooltip or label that advertises it, and the list in `main/help.rs`.
- Interactive elements are real `<button>`, `<input>`, `<dialog>`. No click
  handlers on `div`. Icon-only buttons use `IconButton` with a label.
- Errors render with `role="alert"`; passive status with `role="status"`.
- Classes only: no inline styles, no colour or pixel values in Rust. The one
  exception: a look's preview colours are SVG `fill` attributes.
- State reaches CSS through ARIA or `data-*` attributes (`aria-checked`,
  `data-selected`), not through extra class toggling.

## Text and language

- Every string a person reads comes from `locales/` through `I18n` or a
  `Message`. A line that stays on screen (a toast, a status) is stored as a
  `Message`, not as finished text, so it follows a language change.
- Messages from the shell (`CommandError.message`) are shown as they are.
- File names, key names and the word typed to confirm clearing data are
  not translated.

## Main window specifics

- `Main` (in `windows/main/mod.rs`) holds the window's signals; its methods
  are the actions. Sections take it by value.
- Lists refetch on `notes-changed` and are guarded by a request counter:
  answers can arrive out of order.
- Settings apply as they change. A refused change leaves the old value and
  shows the shell's reason; there is no Save button.
- An AI key goes through `ai_key_set` only. It is never put in a signal that
  outlives its field, and never in the settings.
- The look editor never judges a look: `look_preview` does.

## Details window specifics

- The description and title autosave through `DraftSync`, one per note.
  Closing and switching notes await `flush()` first.
- A refetch replaces a field only when it holds nothing unsaved
  (`logic::details::field_after_load`).
- Attachments arrive by the Add files dialog, drag and drop (paths go to
  the shell, which reads the files) or paste (image bytes as a raw IPC
  body). Previews are `data:` URLs: the CSP does not allow `blob:`.

## Capture window specifics

- The textarea has focus whenever the window is shown. Nothing may steal it.
- Draft writes go through `DraftSync` only. A save calls `cancel()` and
  awaits it before `capture_save`, so a late draft write cannot resurrect
  text that was just filed.
- Nothing is added to the capture window without asking. It is a command
  field, not a form.

## Today window specifics

- Past days are read-only and are rebuilt from timestamps; nothing is
  written to show them. There are no future days.
- A refresh keeps the selection and redraws only rows whose note changed
  (`row_key`), so focus and scroll survive `notes-changed`.
- Keys are handled on the window, not the panel: focus can fall to the page
  when a focused button goes away, and Esc must still close.

## Errors and panics

- No `unwrap()` or `expect()` outside tests. A panic in WebAssembly kills the
  window, with the user's text in it.
- No `RefCell` borrow held across an `.await`.
- Never log note content, drafts or search text.

## Tests

- `logic/` is tested with plain `cargo test -p posthaste-ui`.
- Behaviour is tested in a real browser: `npm run test:ui` runs
  `tests/*.test.mjs` against the built bundle with `tests/harness.mjs`
  standing in for the shell. Query by role and accessible name. Every
  user-visible behaviour gets a test, including its failure path.
- These tests need the `playwright-core` package and a local Chrome.

## Building

`npm run build:ui` (`scripts/build-ui.sh`) needs the `wasm32-unknown-unknown`
target and `wasm-bindgen-cli` at the version pinned in `Cargo.toml`. The two
versions must match; change them together, here and in the release workflow.
