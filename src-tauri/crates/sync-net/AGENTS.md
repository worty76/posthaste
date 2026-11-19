# crates/sync-net: peer-to-peer transport (iroh)

Owns the iroh QUIC endpoint, single-use pairing codes, and length-prefixed
frame I/O on ALPN `posthaste/sync/1`.

## Rules

- **Never open SQLite here.** This crate moves bytes and frames; storage and
  merge rules live in `crates/core`.
- **No n0 lookup services.** Endpoints always start from `presets::Minimal`
  with either `RelayMode::Disabled` or `RelayMode::Custom` for the user's
  configured relay URL.
- **Never log payloads, pairing codes, or secret keys.** Log shortened device
  ids (8 characters), counts, and error kinds only.
- **No `unwrap()` or `expect()` outside tests.** Return `NetError`.
