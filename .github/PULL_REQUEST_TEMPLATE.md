## What and why

<!-- One or two sentences. Link the issue: Fixes #123 -->

## Checks

<!-- Tick what you ran. Leave the rest unticked and say "not run". -->

- [ ] `npm run check`
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `npm run test:ui` (if you changed `ui/`)

## Checklist

- [ ] One change only: no unrelated refactors, renames or dependency upgrades
- [ ] A test covers the change, or I explain why it cannot
- [ ] New interface text is in `locales/en/`, and I added no key without a value
- [ ] No network calls, and nothing logs note content
- [ ] I ran the app on: <!-- macOS / Windows / Linux, or "not run" -->

## Screenshots

<!-- For anything the user can see. Delete this section otherwise. -->
