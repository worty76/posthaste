<!-- Generated from AGENTS.md by scripts/sync-rules.sh. Edit AGENTS.md, then run `npm run sync:rules`. -->

# crates/i18n: interface text

Every string the interface shows, in every language. The text is JSON in
`locales/<code>/` at the repository root; `build.rs` checks it and embeds it.
The desktop shell and the Rust interface both read this crate, so there is
one copy of every message.

## Rules

- English (`locales/en/`) is the source of truth. A message is added there
  first; its key becomes a constant in `key`, so a wrong key does not compile.
- One file per area (`capture.json`, `common.json`); the file name is the
  first part of the key. Nest with objects; keys are `snake_case`.
- A message is a whole sentence with `{placeholders}`. Translators may move a
  placeholder; they may not add, drop or rename one. The build enforces this.
- Do not put text for one programming language's side in a separate place.
  Split files by what the text is about, never by who reads it.
- No plural forms yet. When the first message needs one, add plural support
  here (CLDR categories), not in a window.
- This crate does no I/O and has no dependencies at run time.

## Adding a language

1. Copy `locales/en/` to `locales/<code>/` and translate the values.
2. Set `name` (the language's own name) and `dir` in its `meta.json`.
3. `cargo test -p posthaste-i18n`: `every_language_is_complete` lists
   anything untranslated.

Nothing in Rust or TypeScript names a language, so there is nothing else to
edit. An untranslated key shows the English text.
