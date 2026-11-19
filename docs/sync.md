# Device Sync Architecture & Overview

Posthaste syncs notes directly between your own computers with no central
account or server holding user data. The feature is **experimental** and **off
by default** under **Settings → Devices**.

---

## 1. Identity & Security

- Each computer generates a 32-byte Ed25519 private key the first time sync is
  enabled. The key is stored in the OS credential store (macOS Keychain,
  Windows Credential Manager, Linux Secret Service) under `sync-device-key`,
  falling back to SQLite only when no keychain is available.
- The public half of that key is the computer's **device ID** (`EndpointId`).
- Connections use **iroh** (QUIC + TLS 1.3), so traffic is end-to-end encrypted
  and mutually authenticated by device keys.
- Only **paired** devices may exchange data. Any unpaired device that connects
  is refused before any note is read from SQLite.

## 2. Pairing Two Computers

1. On Computer A, click **Pair a computer** to generate a single-use,
   short-lived code (`posthaste-pair:1:…`) containing:
   - A's device ID
   - A's current local IP:port addresses
   - A's configured relay URL (if any)
   - A random 16-byte one-time secret
   - An expiry timestamp (10 minutes)
   The secret is held **in memory only** on Computer A and is dropped after one
   successful join or after 10 minutes.
2. On Computer B, paste the code into **Have a code?** and click **Pair**. B
   dials A over QUIC and presents the secret in `Hello`. A verifies and consumes
   the secret, both computers record each other in `sync_peers`, and an initial
   full sync runs immediately.

## 3. How Notes Stay in Sync

### Per-field tracking & Hybrid Logical Clock (HLC)
- Every note mutation in `posthaste-core` (`create`, `capture`, `update`,
  `set_status`, `set_later`, `trash`, `restore`, `set_details`, `import`,
  `purge`) records which fields changed inside the **same SQLite transaction**
  in `sync_fields`.
- Tracked fields: `content`, `details`, `category`, `status` (bundled with
  `completed_at` and `archived_at`), `later`, `deleted`, `created`, and
  `purged` (tombstone).
- Each change is stamped with:
  - A **Hybrid Logical Clock (HLC)** (`{wall_ms:013}-{counter:06}-{device}`)
    so string order equals causal order and the clock never moves backwards
    even if a computer's wall clock is off.
  - A local monotonic sequence number (`seq`).

### Reliable sessions
- The transport-agnostic `Session` state machine (`crates/core/src/sync/session.rs`)
  exchanges length-prefixed JSON frames (max 1 MiB) in batches of 500 changes.
- A receiver sends `Ack` **only after SQLite commits** the incoming batch.
- A sender advances `sent_up_to` **only after receiving `Ack`**.
- If the network drops mid-sync, the next session resumes cleanly with no
  duplicates and no skipped changes.

### Merging & never losing user text
- Fields merge independently using **last writer wins** (highest HLC).
- If both computers edit the same note's `content` or `details` while apart,
  the newer edit stays on the note and the losing edit is saved as a new inbox
  note with `source = 'sync-conflict'`.
- If one computer permanently purges a note from Trash while the other edited
  it offline, the note stays purged and the newer text is saved as a
  `sync-conflict` note.

## 4. Background Service & Connections

- When sync is turned off, the iroh endpoint is closed and nothing listens on
  the network.
- When sync is enabled, `SyncService` (`src-tauri/src/sync.rs`) syncs with
  every paired computer:
  1. On app startup
  2. **2 seconds** after a local note write (debounced)
  3. Every **60 seconds** in the background
  4. Immediately when clicking **Sync now**
- **Same Wi-Fi / LAN:** iroh connects directly computer-to-computer over UDP
  QUIC.
- **Different networks:** when direct UDP cannot connect through NATs, iroh
  routes the end-to-end encrypted QUIC traffic through the configured relay
  (`https://relay.posthaste.ink` by default, or empty for same-network only).
  See [relay.md](./relay.md) for self-hosting the relay. The relay sees only
  device IDs and IP addresses, never note content.

## 5. Scope (Current Experimental Version)

- **Synced:** notes and their text/status fields (`content`, `details`,
  `category`, `status`, `later`, `deleted`, `purged`).
- **Not synced yet:** attachments, looks and look images, settings, and unsaved
  drafts.
