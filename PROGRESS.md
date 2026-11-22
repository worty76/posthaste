# Rulings made while executing the plans

## Plan 1: category grouping (branch feat/category-grouping) — COMPLETE

- Task 2: Ruling: the new `api::categories_counts` wrapper carries
  `#[expect(dead_code)]` in its commit, removed in Task 3 when the picker
  calls it. The plan expected clippy clean at Task 2, but a wrapper with no
  caller trips `-D warnings` in a cdylib crate. Cost if wrong: none.
- Task 3: Ruling: the category filter is kept when switching between the
  note views (Inbox, Today, Later, Archive) and cleared when going to Trash
  or Settings. The plan says "kept when switching views", naming only the
  note views; Trash and Settings cannot show the filter line, so keeping the
  filter there would silently narrow a list with no visible reason. Cost if
  wrong: a user who wanted the filter to persist into a trash search does
  not get it.
- Task 3: Ruling: "cleared when the window is closed" is implemented as
  "cleared when the window is hidden", because the shell never destroys the
  main window (CloseRequested is prevented; it hides or minimises). A
  `visibilitychange` listener takes the filter off. Adds the `VisibilityState`
  feature to the existing web-sys dependency (a feature flag, not a new
  dependency). Not verifiable in the browser harness (a real window cannot
  be hidden from the test), so the listener is code-reviewed only.
- Task 3: Ruling: Esc on a search keeps the category (`clear_search` no
  longer resets every filter), so the documented order "search text first,
  then the category filter, then the selection" is one press at a time.
- Task 3: Ruling: the fake shell's `notes_list` now honours `query.category`
  (exact match), so browser tests exercise the real query shape.
- Environment note: 8 scene-look tests ("draws the backdrop…", "no sticker
  ever covers a control…") fail with Microsoft Edge on pristine main too;
  this machine has no Chrome. Pre-existing, unrelated to both plans.

## Plan 2: steps (branch feat/steps) — COMPLETE

- Migration: `0008_steps.sql` confirmed as the next free number at build
  time (0001–0007 exist; no mobile-plan migration present).
- Task 1: `rank_between` uses `String::from_utf8_lossy` instead of
  `expect`: `check-rules.sh` forbids expect outside tests, and every byte
  comes from the ASCII alphabet, so lossy conversion is exact here.
- Task 2: `NewStep` derives `PartialEq`/`Eq` (the split_paste unit test
  compares lists of them).
- Task 3: Ruling: the export method keeps its existing name `changes_since`
  (the plan called it `export_changes`) and returns a `Changes` struct
  (`changes`, `steps`, `up_to`, `withheld_from`); `apply_changes` takes
  `(changes, steps)`. Smallest change; a rename would have touched every
  existing caller for nothing.
- Task 3: Ruling: batch pages are cut at seq boundaries — `changes_since`
  fetches `limit + 6` rows and trims back to the last whole write. The
  pre-existing doc comment already promised "changes that share the same
  seq are never split across pages"; the code did not actually guarantee
  it. Making it true is what lets a step's creation trio (text, done, rank
  recorded with one seq) travel together.
- Task 3: Ruling: when a receiver applies a step's parts it re-records
  them with one shared local seq per step per batch, so a forwarded
  creation never arrives half-built; the held-steps flush also groups rows
  per step before creating.
- Task 3: Ruling: a peer's `sent_up_to` may advance past withheld step
  changes (ordinary note fields keep syncing — spec rule 6); the
  `steps_resend_from` marker is what guarantees the withheld steps are all
  resent after the peer upgrades, and it is cleared only when a session
  sends Done with nothing in flight.
- Task 6: Ruling: `key_action` takes a third argument `text_empty` (the
  plan's two-argument signature cannot decide "Enter on an empty new line
  leaves without saving"), and `StepKey` gains an `Enter` variant (save and
  open the next step). The plan listed seven variants; the behaviour it
  described needed the eighth.
- Task 6: Ruling: "a trashed note's steps are read-only" in the window —
  Note details never opens a trashed note (existing behaviour: the window
  closes when the note is deleted elsewhere, unchanged here), so the
  read-only guarantee lives in the shell (`note_trashed`, covered by core
  tests). The browser test verifies the window leaves a trashed note.
- Task 6: Ruling: the plan's "accessible label reads 2 of 5 steps done" is
  the badge's aria-label, so the browser tests assert the attribute, not
  text content.
- Flake note: one `npm run test:ui` run showed a 9th failure ("does not
  save a scene it has not loaded yet"); it passes standalone and two full
  consecutive runs show only the 8 known Edge scene failures.
