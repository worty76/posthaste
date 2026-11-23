# Posthaste Release Guide

This guide lists every file, version string, commit, and verification check
required when preparing a new Posthaste release (`vX.Y.Z`). Both humans and AI
assistants must follow this checklist from top to bottom so nothing is missed
between the desktop workspace and the landing page.

`scripts/check-rules.sh` (run by `npm run check`) and
`landing-page/tests/config.test.ts` mechanically enforce that every version
location below stays in sync.

---

## 1. Branching & Commit Structure

1. Branch from an up-to-date `main`:
   ```sh
   git switch main && git pull
   git switch -c release/vX.Y.Z
   ```
2. Keep feature/fix commits separate from the version bump and release page
   commits. A typical release branch ends with two release commits after any
   feature or fix commits:
   - `chore: bump to X.Y.Z` — touches **only** `src-tauri/Cargo.toml` and
     `src-tauri/Cargo.lock`
   - `feat(site): the X.Y.Z release page` — touches `landing-page/src/config.ts`
     (and any other `landing-page/` copy if headline features changed)

> **Important CI rule (`.github/workflows/release.yml`):**
> `landing-page/**`, `docs/**`, `scripts/**`, and `**.md` are in the release
> workflow's `paths-ignore`. The `chore: bump to X.Y.Z` commit (`src-tauri/Cargo.toml`
> and `src-tauri/Cargo.lock`) **must** be included in the push/merge to `main`
> so GitHub Actions builds and publishes the release installers.

---

## 2. Every Place the Version Lives

When bumping from an old version to `X.Y.Z`, update **all** of the following
locations.

### A. Desktop Workspace (`src-tauri/`)

1. **`src-tauri/Cargo.toml`** (line 6, under `[workspace.package]`):
   ```toml
   [workspace.package]
   version = "X.Y.Z"
   ```
   *(Never add a `"version"` field to `src-tauri/tauri.conf.json` or the root
   `package.json` — Tauri reads it from `src-tauri/Cargo.toml`, and
   `scripts/check-rules.sh` fails if either JSON file carries `"version"`.)*

2. **`src-tauri/Cargo.lock`** (all 7 workspace crates):
   Run cargo so `Cargo.lock` updates every workspace member (`posthaste`,
   `posthaste-core`, `posthaste-i18n`, `posthaste-model`, `posthaste-platform`,
   `posthaste-sync-net`, `posthaste-ui`) to `X.Y.Z`:
   ```sh
   (cd src-tauri && cargo check --workspace)
   ```
   Verify with `git diff src-tauri/Cargo.lock` that all 7 workspace packages
   now show `version = "X.Y.Z"`.

### B. Landing Page (`landing-page/src/config.ts`) — All 9 Locations

Every release must update **all 9 places** in `landing-page/src/config.ts`:

1. **`RELEASE` base URL** (line 13):
   ```ts
   const RELEASE = "https://github.com/worty76/posthaste/releases/download/vX.Y.Z";
   ```
2. **`PRODUCT.version`** (line 18):
   ```ts
   version: "X.Y.Z",
   ```
3. **`PRODUCT.downloads` doc comment** (line 26):
   ```ts
   * The vX.Y.Z installers, named the way the release workflow names them.
   ```
4. **macOS `.dmg` URL** (`PRODUCT.downloads.mac`):
   ```ts
   mac: { url: `${RELEASE}/Posthaste_X.Y.Z_aarch64.dmg`, label: "Apple Silicon, .dmg" },
   ```
5. **Windows `.exe` URL** (`PRODUCT.downloads.windows`):
   ```ts
   windows: { url: `${RELEASE}/Posthaste_X.Y.Z_x64-setup.exe`, label: "64-bit installer, .exe" },
   ```
6. **Linux `.AppImage` URL** (`PRODUCT.downloads.linux`):
   ```ts
   linux: { url: `${RELEASE}/Posthaste_X.Y.Z_amd64.AppImage`, label: "x86-64, .AppImage" },
   ```
7. **Linux `.deb` URL** (`PRODUCT.linuxExtras[0]`):
   ```ts
   { url: `${RELEASE}/Posthaste_X.Y.Z_amd64.deb`, label: ".deb" },
   ```
8. **Linux `.rpm` URL** (`PRODUCT.linuxExtras[1]`):
   ```ts
   { url: `${RELEASE}/Posthaste-X.Y.Z-1.x86_64.rpm`, label: ".rpm" },
   ```
   *(Note the hyphen before `X.Y.Z-1` in the `.rpm` filename vs underscores in
   the others.)*
9. **`RELEASE_NOTES["X.Y.Z"]` AND `FALLBACK_RELEASES[0]`**:
   - Add a new `"X.Y.Z": [ ... ]` entry at the **top** of `RELEASE_NOTES`:
     plain, human-readable sentences describing what changed for users (no
     commit prefixes, no internal jargon, no stock marketing phrases).
   - Add `["X.Y.Z", "YYYY-MM-DD"]` (using the release's UTC date) as the
     **first entry** of `FALLBACK_RELEASES`.
     *(Forgetting `FALLBACK_RELEASES` breaks the offline/fallback changelog and
     fails `landing-page/tests/releases.test.ts` and `scripts/check-rules.sh`.)*

---

## 3. Documentation & Site Copy Checklist (When Behaviour Changes)

Before cutting the release commits, check whether any changes since the last
tag affect user-facing documentation:

- **`README.md`**:
  - Did a new feature land? Update **Why Posthaste**.
  - Did a keyboard shortcut or Settings preference change? Update **Shortcuts**.
- **`landing-page/`** (when headline features or shortcuts changed):
  - `landing-page/src/components/ShortcutSheet.tsx` — shortcut table on the website
  - `landing-page/src/lib/blocks.ts` — feature blocks (English & Vietnamese)
  - `landing-page/src/lib/i18n.ts` — hero, local, and sync copy (English & Vietnamese)
  - `landing-page/src/lib/compare.ts` — comparison table (English & Vietnamese; bump `COMPARE_CHECKED_ON` if competitor cells were re-verified)
- **`docs/`**:
  - `docs/architecture.md` — if crates, commands, events, or SQLite migrations changed
  - `docs/platform-notes.md` — if OS behaviour, window handling, or verified platforms changed
  - `docs/sync.md` — if synced fields or sync protocol behaviour changed

---

## 4. Verification Commands (All Must Pass)

Run every command below and confirm all pass before considering a release
branch ready:

```sh
# 1. Formatting, rule checks, and release version synchronization check
npm run check

# 2. Rust formatting, clippy (native workspace + wasm32 UI), and unit tests
(cd src-tauri && cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace)
(cd src-tauri && cargo clippy -p posthaste-ui --target wasm32-unknown-unknown -- -D warnings)

# 3. Build the WebAssembly UI bundle and run browser tests
npm run build && npm run test:ui

# 4. Landing page formatting, unit tests (including release config sync), and TypeScript typecheck
(cd landing-page && npm run format:check && npm test && npx tsc --noEmit)
```

*(On macOS where Google Chrome is not installed in the default path, pass
`CHROME_PATH="/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"`
or your Chromium binary path to `npm run test:ui`.)*

---

## 5. Summary of Past Pitfalls (Now Guarded by `check-rules.sh`)

| Pitfall | How it is prevented now |
|---|---|
| Bumping `src-tauri/Cargo.toml` without updating `src-tauri/Cargo.lock` | `scripts/check-rules.sh` verifies all 7 workspace crates in `Cargo.lock` match `Cargo.toml`. |
| Updating `PRODUCT.version` in `landing-page/src/config.ts` but forgetting `FALLBACK_RELEASES` | `scripts/check-rules.sh`, `releases.test.ts`, and `config.test.ts` fail if `FALLBACK_RELEASES[0]` is not the current version. |
| Updating `RELEASE` in `landing-page/src/config.ts` but leaving the old version in `.dmg`, `.exe`, `.AppImage`, `.deb`, or `.rpm` filenames | `scripts/check-rules.sh` and `config.test.ts` check all 5 installer URLs against the version in `src-tauri/Cargo.toml`. |
| Bumping `src-tauri/Cargo.toml` while leaving `landing-page/src/config.ts` on the previous version (or vice versa) | `scripts/check-rules.sh` and `config.test.ts` cross-check `landing-page/src/config.ts` against `src-tauri/Cargo.toml`. |
| Missing `RELEASE_NOTES["X.Y.Z"]` entry in `landing-page/src/config.ts` | `scripts/check-rules.sh` and `config.test.ts` verify `RELEASE_NOTES` contains a non-empty entry for the current version. |
