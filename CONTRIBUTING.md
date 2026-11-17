# Contributing to Posthaste

Thanks for helping. Posthaste does one job: capture a note in a second and get
out of the way. Changes are judged against that loop, so a change must not make
it slower, need more keystrokes, or make it less reliable.

## Before you start

- **Small fix or translation:** open a pull request directly.
- **New feature or setting:** open an issue first and describe what a user
  would do with it. Some things are out on purpose (accounts, sync, telemetry,
  reminders, folders, statistics); see the "Not in it" section of the
  [README](README.md).
- **Not sure?** Open an issue and ask. It is cheaper than a rewrite.

## Set up

You need Rust (stable), Node 20+ and the
[Tauri 2 system dependencies](https://v2.tauri.app/start/prerequisites/). The
exact commands for Linux, the WebAssembly target and `wasm-bindgen-cli` are in
the README under [Build from source](README.md#build-from-source).

```sh
npm install
npm run tauri dev
```

## Send a change

1. Fork the repo on GitHub and clone your fork.
2. Add this repo as `upstream`:
   `git remote add upstream https://github.com/worty76/Posthaste`
3. Branch from an up-to-date `main`, never on `main` itself:
   `git fetch upstream && git switch -c fix/short-name upstream/main`
4. Make the change, then run the checks below.
5. Push to your fork and open a pull request against `main`.
6. Answer review comments by pushing more commits to the same branch.

Keep each pull request to one thing. Do not mix a fix with a refactor, a rename
or a dependency upgrade.

## Checks

These run on every pull request. Run them yourself first:

```sh
npm run check                                     # formatting and rule checks
cd src-tauri
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Always pass `--workspace` to clippy. Without it only one crate is checked.

`npm run test:ui` runs the browser tests of the interface. It needs
`playwright-core` and a Chrome binary, so it is not part of the automatic
checks yet. Run it if you changed anything in `ui/`.

## The rules

The rules live in `AGENTS.md` at the root and in each source folder. Read the
root file and the one nearest the code you change. The ones that matter most:

- Never lose the user's text: a note is saved only after SQLite commits.
- Never send anything off the machine, and never log note content.
- Never edit a shipped migration; add a new numbered one.
- Do not add a dependency without asking in the issue first.
- Never weaken a test to make it pass.
- Read reactive values with `.with(|_| ())`, not `.track()`.

`AGENTS.md` is the source of truth. Each `CLAUDE.md` is a generated copy: edit
`AGENTS.md`, then run `npm run sync:rules`.

## Translations

Interface text lives in `locales/<code>/`, one folder per language. Copy
`locales/en`, translate the values (never the keys or the `{placeholders}`),
and set the language name in `meta.json`. The build checks every language
against English: an unknown key or different placeholders fail the build, and a
missing translation falls back to English.

## Commits and pull requests

- Write the commit message as `type(scope): what changed`, for example
  `fix(ui): switching the look recolours every window again`.
- Say in the pull request what you changed and why, and which checks you ran.
  Write "not run" for the ones you did not, and do not claim a platform works
  unless you ran the app on it.
- For visible changes, add a screenshot.

By contributing you agree that your work is released under the
[MIT licence](LICENSE).
