//! Records local note mutations in `sync_fields` inside the caller's SQLite
//! transaction.

use rusqlite::{params, Transaction};

use crate::error::Result;
use crate::sync::hlc::Hlc;
use crate::sync::merge::StepPart;
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
    let Some(device_id) = state_get(tx, STATE_DEVICE_ID)? else {
        return Ok(());
    };
    if fields.is_empty() {
        return Ok(());
    }
    record_with(
        tx,
        &device_id,
        note_id,
        fields
            .iter()
            .map(|f| (f.as_str().to_owned(), None))
            .collect::<Vec<_>>()
            .as_slice(),
        now_ms,
    )
}

/// Records step fields of one step. All parts of one write share one HLC
/// tick and one `seq`, so a peer's batch never splits a step's creation.
pub(crate) fn record_step_parts(
    tx: &Transaction<'_>,
    note_id: &str,
    step_id: &str,
    parts: &[StepPart],
    now_ms: i64,
) -> Result<()> {
    let Some(device_id) = state_get(tx, STATE_DEVICE_ID)? else {
        return Ok(());
    };
    if parts.is_empty() {
        return Ok(());
    }
    let fields: Vec<(String, Option<String>)> = parts
        .iter()
        .map(|part| (step_field_name(step_id, *part), None))
        .collect();
    record_with(tx, &device_id, note_id, &fields, now_ms)
}

fn record_with(
    tx: &Transaction<'_>,
    device_id: &str,
    note_id: &str,
    fields: &[(String, Option<String>)],
    now_ms: i64,
) -> Result<()> {
    let last_clock = state_get(tx, STATE_CLOCK)?.and_then(|s| Hlc::parse(&s));
    let hlc = Hlc::tick(last_clock.as_ref(), now_ms, device_id);
    let hlc_str = hlc.format();
    state_set(tx, STATE_CLOCK, &hlc_str)?;
    let seq = next_seq(tx)?;

    for (field, _) in fields {
        tx.execute(
            "INSERT INTO sync_fields (note_id, field, hlc, seq)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(note_id, field) DO UPDATE SET
                 hlc = excluded.hlc,
                 seq = excluded.seq",
            params![note_id, field, hlc_str, seq],
        )?;
    }
    Ok(())
}

/// The `sync_fields` name of one step part: `step:<id>:<part>`.
pub(crate) fn step_field_name(step_id: &str, part: StepPart) -> String {
    format!("step:{step_id}:{}", part.as_str())
}
