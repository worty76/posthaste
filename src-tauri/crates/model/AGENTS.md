# crates/model: shared data shapes

The serde types storage writes, the shell sends over IPC and the Rust
interface reads. One definition, so the sides of the IPC boundary cannot
drift apart.

- `domain.rs`: what is stored (notes, drafts, settings).
- `wire.rs`: what exists only on the boundary (command results, event
  payloads).
- Plain data only: no I/O, no SQL, no Tauri, no browser types. The crate
  must build for `wasm32-unknown-unknown`.
- Every type derives both `Serialize` and `Deserialize`: each crosses the
  boundary in one direction and is read back on the other side.
- While some windows are still React, `src/lib/types.ts` mirrors these types
  by hand. Change both in the same change.
- Settings must keep loading: a new field needs a default, and a value this
  version does not know must not reset the others.
