<!-- Generated from AGENTS.md by scripts/sync-rules.sh. Edit AGENTS.md, then run `npm run sync:rules`. -->

# crates/core: storage

SQLite schema, migrations, notes, drafts, search, settings, export/import.
Everything that decides what valid data is lives here. This crate has no Tauri,
window or UI dependency, and must stay that way: it is tested against a plain
database file in milliseconds.

## Files

| File | Owns |
|---|---|
| `db.rs` | `Db`: the connection and every query |
| `migrations.rs` + `migrations/*.sql` | Schema history |
| `steps/` | Steps inside a note: storage, ranks, limits. Tests in `steps/tests.rs` |
| `transfer.rs` | JSON export and import |
| `look/` | Looks: document validation, token derivation, share codes, storage. Tests in `look/tests/` |
| `sync/` | Device sync engine: HLC, per-field change tracking, LWW merge with conflict preservation, transport-agnostic session. Tests in `sync/tests/` |
| `error.rs` | `CoreError` and its stable `code()` |
| `tests.rs` | Tests for db, model and migrations |

The data types themselves live in `crates/model` and are re-exported from
here, so storage code keeps writing `posthaste_core::Note`.

Look-specific rules: look validation and derivation live only in `look/`
(`mod.rs` validates the one format and the font name, `derive.rs` derives the
token map, `scene.rs` checks layers against their kind's limits and regions);
the bundled looks are data files in `crates/core/looks/`, parsed by
`look/bundled.rs` and kept valid by tests. A user look is stored as its JSON
text in the `looks` table; unreadable rows are skipped, never trusted.

The look format keeps unknown data: a document may carry fields and layer
kinds this build does not model, and every save path (`save_look`,
`import_look`, the share-code paths) must keep them with the same content
(key order is not kept). Never rebuild a stored document from typed structs
— edit the parsed value. The fixture tests in `look/tests/` fail the build
if a save path drops data. A match on `Known` may not have a wildcard arm;
`scripts/check-rules.sh` enforces it.

Look images live in SQLite (`look_assets`), content-addressed by the SHA-256
of their bytes, accepted only by header sniffing (PNG, JPEG, WebP) and size
checks — never by file name. Look files (`.phlook`, `look/package.rs`)
refuse anything they cannot verify before anything is written, and
`install_package` stores images and look in one transaction. Simple looks is
applied here, in `active_scene`, so no window can forget to obey it. The
UI never parses a look or reads a file; it draws `ResolvedScene` and edits
through `ThemeEdit` and `SceneEdit` the store hands back.

Look invariants, in every phase (a look is data, never code, and the
user's settings always beat it):

1. A look cannot run code: no scripts and no CSS text. Images come only
   from the look's own checked assets. A font is only the name of one
   installed on this computer, checked against the font-name rule; a look
   never carries or fetches font files.
2. Preserved unknown data is inert. It is stored and exported, never drawn
   and never turned into CSS.
3. Decorations never take input: `pointer-events: none` on every scene
   layer, and no focusable element inside one.
4. Margin layers stay in the margin. The renderer clamps them to the
   window's edge band, so they cannot cover the text field or a note.
5. A backdrop never lowers text contrast. Content keeps its `--surface`
   panel at 85% opacity or more over any backdrop image.
6. Reduce motion and Simple looks always win. Simple looks keeps the
   colours and turns off every scene layer, in every window.
7. One bad layer never breaks a window. A layer that fails its check is
   skipped with a warning, the same way a bad stored look is today.

## Data safety

- Every write that touches more than one row or table is one transaction.
  Capture is the model: insert the note and delete the draft together.
- Every note write records its changed fields in `sync_fields` inside the same
  transaction (`sync::record::record`). A failed write records nothing.
- Inputs are cleaned at the boundary (`clean_content`, `clean_category`) and
  also constrained in the schema with `CHECK`. Keep both; the schema is the
  last line of defence against a bug in the Rust code.
- Deleting a note is a soft delete (`deleted_at`). Every read filters
  `deleted_at IS NULL`. A new query that forgets this resurrects deleted notes.
  The only reads of trashed notes are the Trash ones, and they also skip notes
  older than `TRASH_RETENTION_MS`, which can no longer be restored.
- Import never overwrites. Export excludes trashed notes.
- `synchronous = FULL` and WAL stay. Do not trade durability for speed.

## Migrations

- **Append-only.** Never edit or reorder a file in `migrations/`. Add
  `000N_description.sql` and a matching entry at the end of `MIGRATIONS`.
- Versions are consecutive integers from 1; a test enforces it.
- Each migration must work on a database that already holds data, and is run
  inside a transaction with the version bump.
- Every migration gets a test that starts from the previous schema with rows in
  it, upgrades, and asserts the rows survived (see
  `upgrading_from_v1_keeps_notes_indexes_them_and_leaves_a_snapshot`).
- If a migration changes a column that the full-text index covers, update the
  triggers in the same migration.

## Queries

- All SQL uses bound parameters. Never format user input into SQL. The dynamic
  parts of `list_notes` build placeholders, not values.
- Search text goes through `fts_query`, which tokenises it. Raw user text never
  reaches `MATCH`.
- A new filter or sort gets an index if it would otherwise scan, and the
  large-collection test (`search_stays_fast_with_a_large_collection`) is the
  model for proving it.
- Timestamps are UTC milliseconds (`i64`). The caller supplies local-day
  bounds; this crate never guesses a time zone.

## API shape

- Public methods come in pairs where time matters: `create_note(new)` calls
  `create_note_at(new, now_ms())`. Tests use the `_at` form. Never call the
  system clock inside logic that a test needs to control.
- Return `Result<T>` with a `CoreError`. A new failure kind gets a variant, a
  stable `code()`, and a message a person can act on.
- A type sent to the interface lives in `crates/model`, not here, and derives
  both `Serialize` and `Deserialize`.
- `Settings` uses `#[serde(default)]`: a new field must have a default so
  settings saved by older versions still load. Never rename or repurpose an
  existing field; add a new one.

## Tests

- Every public method has tests for the normal case, the empty or missing
  case, and the failure case.
- Tests use `Db::open_in_memory()` unless the behaviour is about files
  (reopen, crash, backup, migration snapshot); those use `tempfile`.
- Test names are sentences about behaviour.
