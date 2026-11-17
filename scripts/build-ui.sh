#!/usr/bin/env bash
# Builds the Rust interface (ui/) to WebAssembly and writes the loadable
# module to ui/pkg/, where ui/boot.js imports it.
#
#   scripts/build-ui.sh          optimised build, for `npm run build`
#   scripts/build-ui.sh --dev    fast unoptimised build, for `npm run dev`
#
# Needs, once per machine:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version 0.2.130 --locked
# The CLI version must equal the `wasm-bindgen` version pinned in ui/Cargo.toml.
set -euo pipefail
cd "$(dirname "$0")/.."

WASM_BINDGEN_VERSION="0.2.130"
PROFILE_DIR=release
PROFILE_FLAG=--release
if [ "${1:-}" = "--dev" ]; then
  PROFILE_DIR=debug
  PROFILE_FLAG=
  # wasm-bindgen re-encodes the module and inflates it several-fold: left
  # unoptimised, every window loaded a 117 MB module. One optimisation pass
  # keeps it around 23 MB. Overflow checks stay on, as dev expects.
  export CARGO_PROFILE_DEV_OPT_LEVEL=1
  export CARGO_PROFILE_DEV_OVERFLOW_CHECKS=true
fi

if ! command -v wasm-bindgen >/dev/null; then
  echo "wasm-bindgen is not installed. Run: cargo install wasm-bindgen-cli --version $WASM_BINDGEN_VERSION --locked" >&2
  exit 1
fi
FOUND="$(wasm-bindgen --version | awk '{print $2}')"
if [ "$FOUND" != "$WASM_BINDGEN_VERSION" ]; then
  echo "wasm-bindgen $FOUND is installed but ui/Cargo.toml pins $WASM_BINDGEN_VERSION. Run: cargo install wasm-bindgen-cli --version $WASM_BINDGEN_VERSION --locked" >&2
  exit 1
fi

# UI_CARGO_ARGS: extra cargo arguments, for machines with an unusual toolchain.
# shellcheck disable=SC2086
(cd src-tauri && cargo build -p posthaste-ui --target wasm32-unknown-unknown $PROFILE_FLAG ${UI_CARGO_ARGS:-})

wasm-bindgen --target web --no-typescript \
  --out-dir ui/pkg --out-name posthaste_ui \
  "src-tauri/target/wasm32-unknown-unknown/$PROFILE_DIR/posthaste_ui.wasm"
echo "ui: built ($PROFILE_DIR) -> ui/pkg"
