<!-- Generated from AGENTS.md by scripts/sync-rules.sh. Edit AGENTS.md, then run `npm run sync:rules`. -->

# ui/: the Rust interface

Posthaste's windows, written in Rust with Leptos and compiled to WebAssembly.
Windows move here from `src/` (React) one at a time; the table in the root
`AGENTS.md` says which have moved. Like `src/`, this code renders state and
handles input. It decides nothing about what a valid note is or how it is
stored; `crates/core` owns that.

## Structure

```
src/lib.rs        entry point; the module table at its top is the map
src/logic/        decisions as plain functions. No browser, no Leptos
src/ipc/          every command and event. The only code that touches Tauri
src/state/        settings, language, look: shared by windows
src/components/   controls that know nothing about notes
src/windows/      one module per window
src/draft_sync.rs the autosave queue
boot.js           loads the stylesheets and starts the module
tests/            browser tests against a fake shell
pkg/              build output (ignored by git)
```

Dependencies point one way: `windows` → `state`, `components`, `ipc`,
`logic`. `logic` imports none of the others. `components` never imports
`ipc`; a control that needs a command belongs in `windows`.

## Where code goes

- Can it be decided from plain values (a key press and the settings, an
  accelerator string, a theme)? It goes in `logic/` as a function with unit
  tests, and the window calls it. `logic/capture.rs` is the model.
- A window keeps its signals in one `Copy` struct and its actions as methods
  on it (`Capture` in `windows/capture.rs`). Handlers call those methods; the
  `view!` holds markup and wiring only.
- A new window: one module in `windows/`, one arm in `windows::mount`, its
  stylesheet imported in `boot.js`, and its URL changed to `RUST_UI` in
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

## Keyboard, accessibility, styling

The rules in `src/AGENTS.md` hold here unchanged: every action by keyboard,
nothing fires during IME composition, `has_mod` for the primary modifier,
real `<button>`/`<input>` elements, `role="alert"` for errors, classes only.
The stylesheets are the shared ones in `src/styles/`; this folder has none.

## Capture window specifics

- The textarea has focus whenever the window is shown. Nothing may steal it.
- Draft writes go through `DraftSync` only. A save calls `cancel()` and
  awaits it before `capture_save`, so a late draft write cannot resurrect
  text that was just filed.
- Nothing is added to the capture window without asking. It is a command
  field, not a form.

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
