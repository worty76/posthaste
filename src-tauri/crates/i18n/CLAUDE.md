<!-- Generated from AGENTS.md by scripts/sync-rules.sh. Edit AGENTS.md, then run `npm run sync:rules`. -->

# crates/i18n: interface text

Every string the interface shows, in every language. The text is JSON in
`locales/<code>/` at the repository root; `build.rs` checks it and embeds it.
The desktop shell and the Rust interface both read this crate, so there is
one copy of every message.

## Rules

- English (`locales/en/`) is the source of truth. A message is added there
  first; its key becomes a constant in `key`, so a wrong key does not compile.
- One file per area (`capture.json`, `desktop.json` for the tray menu and
  file dialogs); the file name is the
  first part of the key. Nest with objects; keys are `snake_case`.
- A message is a whole sentence with `{placeholders}`. Translators may move a
  placeholder; they may not add, drop or rename one. The build enforces this.
- Do not put text for one programming language's side in a separate place.
  Split files by what the text is about, never by who reads it.
- A message that changes with a number is an object of plural categories
  (`{"one": "{count} file", "other": "{count} files"}`), read with
  `Translator::plural`. Every language writes `other`; the rest follow its
  own rule, named by `plural` in its `meta.json`. A language whose rule is
  not in `PluralRule` yet needs a variant added there, never a special case
  in a window.
- A control in the middle of a sentence: keep one message with a
  placeholder and split it with `Translator::around`.
- This crate does no I/O and has no dependencies at run time.

## Adding a language

1. Copy `locales/en/` to `locales/<code>/` and translate the values.
2. Set `name` (the language's own name), `dir` and `plural` in its
   `meta.json`.
3. `cargo test -p posthaste-i18n`: `every_language_is_complete` lists
   anything untranslated.

Nothing in the code names a language, so there is nothing else to
edit. An untranslated key shows the English text.
