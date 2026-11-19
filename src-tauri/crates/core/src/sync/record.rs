//! Records local note mutations in `sync_fields` inside the caller's SQLite
//! transaction.

use rusqlite::{params, Transaction};

use crate::error::Result;
use crate::sync::hlc::Hlc;
use crate::sync::store::{next_seq, state_get, state_set, Field, STATE_CLOCK, STATE_DEVICE_ID};

/// Records that `fields` on `note_id` changed at `now_ms`.
///
/// No-op if sync has not been initialised on this database (no `device_id` in
/// `sync_state`). Otherwise ticks the local HLC once, allocates one new `seq`
/// per call, and upserts every listed field with that `(hlc, seq)`.
pub(crate) fn record(
    tx: &Transaction<'_>,
    note_id: &str,
    fields: &[Field],
    now_ms: i64,
) -> Result<()> {
    if fields.is_empty() {
        return Ok(());
    }
    let Some(device_id) = state_get(tx, STATE_DEVICE_ID)? else {
        return Ok(());
    };
    let last_clock = state_get(tx, STATE_CLOCK)?.and_then(|s| Hlc::parse(&s));
    let hlc = Hlc::tick(last_clock.as_ref(), now_ms, &device_id);
    let hlc_str = hlc.format();
    state_set(tx, STATE_CLOCK, &hlc_str)?;
    let seq = next_seq(tx)?;

    for field in fields {
        tx.execute(
            "INSERT INTO sync_fields (note_id, field, hlc, seq)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(note_id, field) DO UPDATE SET
                 hlc = excluded.hlc,
                 seq = excluded.seq",
            params![note_id, field.as_str(), hlc_str, seq],
        )?;
    }
    Ok(())
}
