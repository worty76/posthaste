#!/usr/bin/env bash
# Mechanical checks for the rules in the AGENTS.md files. Rules that a grep
# can enforce live here, so they fail the build instead of relying on review.
# Run via `npm run check:rules` (part of `npm run check`).
set -u
cd "$(dirname "$0")/.."
FAIL=0
rule() { # rule "<message>" "<offending lines>"
  if [ -n "$2" ]; then FAIL=1; printf '\n✗ %s\n%s\n' "$1" "$2"; fi
}

rule "Raw colours are only allowed in ui/styles/tokens.css (ui/styles/AGENTS.md)" \
  "$(grep -rnE '#[0-9a-fA-F]{3,8}\b|\brgba?\(|\bhsla?\(' ui/styles --include='*.css' \
      | grep -v '^ui/styles/tokens.css:')"

rule "Inline styles are not allowed; use a class and tokens (ui/AGENTS.md)" \
  "$(grep -rnE '\bstyle(=|:[a-z-]+=)' ui/src --include='*.rs')"

# One exception: capture replays its entrance animation by clearing it.
rule "set_property is only allowed in ui/src/state/appearance.rs (ui/AGENTS.md)" \
  "$(grep -rn 'set_property' ui/src --include='*.rs' | grep -v '^ui/src/state/appearance.rs:' \
      | grep -v 'set_property("animation", "none")')"

# The dark and the light block must define exactly the same token names, so
# a look can swap either base without a missing token.
DARK_TOKENS=$(awk '/^:root,$/,/^}/' ui/styles/tokens.css | grep -oE '^\s*--[a-z0-9-]+' | sort)
LIGHT_TOKENS=$(awk '/^:root\[data-theme="light"\]/,/^}/' ui/styles/tokens.css | grep -oE '^\s*--[a-z0-9-]+' | sort)
rule "Every token must be defined in both the dark and the light block of ui/styles/tokens.css (ui/styles/AGENTS.md)" \
  "$(diff <(echo "$DARK_TOKENS") <(echo "$LIGHT_TOKENS"))"

rule "The interface is Rust: no TypeScript or JSX outside landing-page/ (AGENTS.md)" \
  "$(git ls-files '*.ts' '*.tsx' '*.jsx' | grep -v '^landing-page/')"

rule "ui/boot.js and the test harness are the only hand-written JavaScript (ui/AGENTS.md)" \
  "$(git ls-files '*.js' '*.mjs' | grep -vE '^(landing-page/|ui/boot\.js$|ui/tests/|vite\.config\.js$)')"

rule "Browser storage is not used; state lives in SQLite (AGENTS.md)" \
  "$(grep -rnE '\b(localStorage|sessionStorage|indexedDB|local_storage|session_storage)\b' ui/src ui/boot.js)"

rule "No network calls from the UI (AGENTS.md)" \
  "$(grep -rnE '\bfetch\(|XmlHttpRequest|XMLHttpRequest|WebSocket|https?://' ui/src ui/boot.js \
      | awk -F: '{ if (system("awk \"NR<" $2 " && /#\\[cfg\\(test\\)\\]/ {f=1} END {exit !f}\" " $1) != 0) print }')"

rule "posthaste-core must not depend on Tauri (src-tauri/crates/core/AGENTS.md)" \
  "$(grep -nE '^\s*tauri' src-tauri/crates/core/Cargo.toml)"

rule "Shipped migrations are append-only: do not edit, add a new numbered file (src-tauri/crates/core/AGENTS.md)" \
  "$(git diff --name-only --diff-filter=MD HEAD -- src-tauri/crates/core/migrations 2>/dev/null)"

rule "Only ui/src/ipc may touch Tauri (ui/AGENTS.md)" \
  "$(grep -rnE '__TAURI|tauri_invoke|tauri_listen' ui/src --include='*.rs' | grep -v '^ui/src/ipc/')"

rule "ui/src/logic is plain Rust: no Leptos, browser or IPC imports (ui/AGENTS.md)" \
  "$(grep -rnE 'leptos|web_sys|wasm_bindgen|js_sys|crate::(ipc|state|windows|components)' ui/src/logic --include='*.rs')"

rule "Only ui/src/time.rs reads the clock or formats dates (ui/AGENTS.md)" \
  "$(grep -rnE 'js_sys::Date|Date::(new|now)|DateTimeFormat' ui/src --include='*.rs' | grep -v '^ui/src/time.rs:')"

rule "ui/src/components must not import ipc (ui/AGENTS.md)" \
  "$(grep -rn 'crate::ipc' ui/src/components --include='*.rs')"

rule "The wasm-bindgen version must be the same in ui/Cargo.toml, scripts/build-ui.sh and the release workflow (ui/AGENTS.md)" \
  "$(V=$(grep -oE '^wasm-bindgen = "=[0-9.]+"' ui/Cargo.toml | grep -oE '[0-9.]+'); \
     grep -q "WASM_BINDGEN_VERSION=\"$V\"" scripts/build-ui.sh || echo "scripts/build-ui.sh does not pin $V"; \
     grep -q "wasm-bindgen-cli --version $V " .github/workflows/release.yml || echo ".github/workflows/release.yml does not install $V")"

rule "Read a reactive value with .with(|_| ()) instead of .track(): a Memo that is only tracked is never computed and never updates (ui/AGENTS.md)" \
  "$(grep -rn '\.track()' ui/src --include='*.rs')"

rule "No unwrap()/expect() in non-test Rust; return an error (src-tauri/AGENTS.md)" \
  "$(grep -rnE '\.unwrap\(\)|\.expect\(' src-tauri/src src-tauri/crates/*/src ui/src --include='*.rs' \
      | grep -v '/tests.rs:' | grep -v '/tests/' | grep -v 'could not start' \
      | awk -F: '{ if (system("awk \"NR<" $2 " && /#\\[cfg\\(test\\)\\]/ {f=1} END {exit !f}\" " $1) != 0) print }')"

rule "The version lives only in src-tauri/Cargo.toml [workspace.package] (src-tauri/AGENTS.md)" \
  "$(grep -n '"version"' src-tauri/tauri.conf.json package.json)"

rule "CLAUDE.md copies are out of date; run: npm run sync:rules (AGENTS.md)" \
  "$(bash scripts/sync-rules.sh --check)"

# A match on Known must handle every layer kind explicitly: a wildcard arm
# would let a new kind be silently skipped by the checker or the renderer.
rule "No match on Known may have a _ => arm (crates/core/AGENTS.md)" \
  "$(grep -rn '_ =>' src-tauri/crates/core/src/look/scene.rs src-tauri/crates/core/src/look/resolve.rs ui/src/components/scene 2>/dev/null)"

# Looks have one format. The v1 names and wire values are gone; anything
# reintroducing them (a type, a converter, a :1: code) must fail the check.
rule "Looks have one format; no v1 remnants (crates/core/AGENTS.md)" \
  "$(grep -rnE 'LookDocV2|LookDocument|v1_view|v1_to_v2|posthaste-look:1' \
      src-tauri ui/src ui/tests landing-page/src \
      --include='*.rs' --include='*.ts' --include='*.tsx' --include='*.mjs' 2>/dev/null)"

# Every bundled font file carries its licence: one OFL-<family>.txt per
# family, committed beside the files it covers.
rule "Every bundled font has its OFL licence beside it (ui/styles/AGENTS.md)" \
  "$(for f in ui/styles/fonts/*.woff2; do
      fam=$(basename "$f" .woff2 | sed -E 's/-[0-9]+$//; s/-variable$//')
      [ -f "ui/styles/fonts/OFL-$fam.txt" ] || echo "$f has no OFL-$fam.txt"
    done)"

# There is no Plain look and no light/dark setting: every install wears a
# look, and light or dark comes from the look's base.
rule "No Plain look and no theme setting remain (crates/core/AGENTS.md)" \
  "$(grep -rn 'LOOKS_PLAIN\|Theme::System' ui/src src-tauri/src src-tauri/crates --include='*.rs' 2>/dev/null)"

if [ "$FAIL" -eq 0 ]; then echo "rules: ok"; fi
exit "$FAIL"
