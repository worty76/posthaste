//! Step-level change export, last-writer-wins merging, and the holding queue
//! for step changes that arrive before their parent note.

use std::collections::BTreeMap;

use posthaste_model::MAX_STEP_CHARS;
use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::Result;
use crate::sync::hlc::Hlc;
use crate::sync::merge::{note_exists, ApplyReport};
use crate::sync::record::step_field_name;
use crate::sync::store::next_seq;

/// One tracked part of one step, exchanged between devices that announced
/// the `steps` capability. Its `sync_fields` name is `step:<id>:<part>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepPart {
    Text,
    Done,
    Rank,
    Deleted,
}

impl StepPart {
    pub fn as_str(self) -> &'static str {
        match self {
            StepPart::Text => "text",
            StepPart::Done => "done",
            StepPart::Rank => "rank",
            StepPart::Deleted => "deleted",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "text" => StepPart::Text,
            "done" => StepPart::Done,
            "rank" => StepPart::Rank,
            "deleted" => StepPart::Deleted,
            _ => return None,
        })
    }
}

/// One per-step change exchanged between devices.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepChange {
    pub note_id: String,
    pub step_id: String,
    pub part: StepPart,
    pub value: Value,
    pub hlc: String,
}

/// Groups step changes by the note they belong to, keeping their order.
pub(super) fn by_step_note(steps: &[StepChange]) -> BTreeMap<&str, Vec<&StepChange>> {
    let mut by_note: BTreeMap<&str, Vec<&StepChange>> = BTreeMap::new();
    for ch in steps {
        by_note.entry(ch.note_id.as_str()).or_default().push(ch);
    }
    by_note
}

/// Incoming step stamps advance the clock and the peer watermark exactly
/// like note stamps do.
pub(super) fn observe_step_clocks(
    clock: &mut Option<Hlc>,
    max_received_hlc: &mut Option<String>,
    step_changes: &[&StepChange],
    local_device: &str,
    now_ms: i64,
) {
    for ch in step_changes {
        if max_received_hlc
            .as_deref()
            .map(|prev| ch.hlc.as_str() > prev)
            .unwrap_or(true)
        {
            *max_received_hlc = Some(ch.hlc.clone());
        }
        if let Some(incoming) = Hlc::parse(&ch.hlc) {
            *clock = Some(Hlc::observe(
                clock.as_ref(),
                &incoming,
                now_ms,
                local_device,
            ));
        }
    }
}

/// Splits a `step:<id>:<part>` field name back into its pieces.
pub fn parse_step_field(field: &str) -> Option<(String, StepPart)> {
    let rest = field.strip_prefix("step:")?;
    let (id, part) = rest.rsplit_once(':')?;
    if id.is_empty() {
        return None;
    }
    Some((id.to_owned(), StepPart::parse(part)?))
}

/// Reads one step part's current value, straight from the step row. A step
/// whose row is gone (its note was purged) exports nothing: the purge
/// record carries the deletion.
pub(super) fn read_step_value(
    conn: &rusqlite::Connection,
    step_id: &str,
    part: StepPart,
) -> Result<Option<Value>> {
    let row: Option<(String, i64, String, Option<i64>)> = conn
        .query_row(
            "SELECT text, done, rank, deleted_at FROM note_steps WHERE id = ?1",
            [step_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let Some((text, done, rank, deleted_at)) = row else {
        return Ok(None);
    };
    Ok(Some(match part {
        StepPart::Text => json!(text),
        StepPart::Done => json!(done != 0),
        StepPart::Rank => json!(rank),
        StepPart::Deleted => json!(deleted_at.is_some()),
    }))
}

/// Whether a step row exists (tombstones included: their fields still merge).
fn step_row_exists(tx: &Transaction<'_>, step_id: &str) -> Result<bool> {
    let count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM note_steps WHERE id = ?1",
        [step_id],
        |r| r.get(0),
    )?;
    Ok(count > 0)
}

/// Applies one note's incoming step changes: last writer wins per part, a
/// step the device has not seen is created from the batch, and everything
/// applied is recorded again with a local seq so it forwards.
pub(super) fn apply_steps_for_note(
    tx: &Transaction<'_>,
    note_id: &str,
    changes: &[&StepChange],
    now_ms: i64,
    report: &mut ApplyReport,
) -> Result<()> {
    let mut by_step: BTreeMap<&str, Vec<&StepChange>> = BTreeMap::new();
    for ch in changes {
        by_step.entry(ch.step_id.as_str()).or_default().push(ch);
    }
    for (step_id, mut group) in by_step {
        group.sort_by_key(|c| match c.part {
            StepPart::Text => 0,
            StepPart::Done => 1,
            StepPart::Rank => 2,
            StepPart::Deleted => 3,
        });
        if !step_row_exists(tx, step_id)? {
            // A step this device has not seen: its creation always travels
            // whole (text included), so the row can be built from the batch.
            if !create_step_from_batch(tx, note_id, step_id, &group, now_ms)? {
                report.skipped += group.len();
                continue;
            }
        }
        // One seq for all parts of this step in this batch, so the whole
        // creation forwards together.
        let seq = next_seq(tx)?;
        for ch in group {
            let name = step_field_name(step_id, ch.part);
            let local_hlc: Option<String> = tx
                .query_row(
                    "SELECT hlc FROM sync_fields WHERE note_id = ?1 AND field = ?2",
                    params![note_id, name],
                    |r| r.get(0),
                )
                .optional()?;
            if local_hlc
                .as_deref()
                .is_some_and(|local| local >= ch.hlc.as_str())
            {
                report.skipped += 1;
                continue;
            }
            if apply_single_step(tx, note_id, step_id, ch, now_ms)? {
                tx.execute(
                    "INSERT INTO sync_fields (note_id, field, hlc, seq)
                     VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT(note_id, field) DO UPDATE SET
                         hlc = excluded.hlc,
                         seq = excluded.seq",
                    params![note_id, name, ch.hlc, seq],
                )?;
                report.applied += 1;
            } else {
                report.skipped += 1;
            }
        }
    }
    Ok(())
}

/// Creates a step row from the values a batch carries. `false` when the
/// batch does not include usable text for it (it will be retried when the
/// creation arrives whole).
fn create_step_from_batch(
    tx: &Transaction<'_>,
    note_id: &str,
    step_id: &str,
    group: &[&StepChange],
    now_ms: i64,
) -> Result<bool> {
    let text = group
        .iter()
        .find(|c| c.part == StepPart::Text)
        .and_then(|c| c.value.as_str())
        .filter(|t| clean_incoming_step_text(t).is_ok());
    let Some(text) = text else {
        return Ok(false);
    };
    let done = group
        .iter()
        .find(|c| c.part == StepPart::Done)
        .and_then(|c| c.value.as_bool())
        .unwrap_or(false);
    let rank = group
        .iter()
        .find(|c| c.part == StepPart::Rank)
        .and_then(|c| c.value.as_str())
        .unwrap_or("")
        .to_owned();
    let now = group
        .iter()
        .find_map(|c| Hlc::parse(&c.hlc).map(|h| h.wall_ms))
        .unwrap_or(now_ms);
    tx.execute(
        "INSERT OR IGNORE INTO note_steps (id, note_id, text, done, rank, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        params![step_id, note_id, text, done as i64, rank, now],
    )?;
    Ok(true)
}

/// Applies one step part. Per-field LWW has already decided the winner.
fn apply_single_step(
    tx: &Transaction<'_>,
    note_id: &str,
    step_id: &str,
    ch: &StepChange,
    now_ms: i64,
) -> Result<bool> {
    let updated_at = Hlc::parse(&ch.hlc).map(|h| h.wall_ms).unwrap_or(now_ms);
    let ok = match ch.part {
        StepPart::Text => {
            let Some(raw) = ch.value.as_str() else {
                return Ok(false);
            };
            let Ok(text) = clean_incoming_step_text(raw) else {
                return Ok(false);
            };
            tx.execute(
                "UPDATE note_steps SET text = ?1, updated_at = ?2 WHERE id = ?3",
                params![text, updated_at, step_id],
            )? > 0
        }
        StepPart::Done => {
            let Some(done) = ch.value.as_bool() else {
                return Ok(false);
            };
            tx.execute(
                "UPDATE note_steps SET done = ?1, updated_at = ?2 WHERE id = ?3",
                params![done as i64, updated_at, step_id],
            )? > 0
        }
        StepPart::Rank => {
            let Some(rank) = ch.value.as_str() else {
                return Ok(false);
            };
            tx.execute(
                "UPDATE note_steps SET rank = ?1, updated_at = ?2 WHERE id = ?3",
                params![rank, updated_at, step_id],
            )? > 0
        }
        // A tombstone is set by any deletion; only an explicit undelete
        // (deleted = false) clears one. Text, done and rank never touch it,
        // so an edit of a deleted step cannot bring it back.
        StepPart::Deleted => {
            let Some(deleted) = ch.value.as_bool() else {
                return Ok(false);
            };
            if deleted {
                tx.execute(
                    "UPDATE note_steps SET deleted_at = COALESCE(deleted_at, ?1), updated_at = ?1
                     WHERE id = ?2",
                    params![updated_at, step_id],
                )? > 0
            } else {
                tx.execute(
                    "UPDATE note_steps SET deleted_at = NULL, updated_at = ?1 WHERE id = ?2",
                    params![updated_at, step_id],
                )? > 0
            }
        }
    };
    if ok {
        tx.execute(
            "UPDATE notes SET updated_at = MAX(updated_at, ?1) WHERE id = ?2",
            params![updated_at, note_id],
        )?;
    }
    Ok(ok)
}

/// Step text is validated again on arrival; a part that fails is skipped,
/// never stored, and the batch keeps going.
fn clean_incoming_step_text(raw: &str) -> std::result::Result<String, ()> {
    let text = raw.trim();
    if text.is_empty()
        || text.chars().count() > MAX_STEP_CHARS
        || text.chars().any(|c| c == '\n' || c == '\r')
    {
        return Err(());
    }
    Ok(text.to_owned())
}

/// Step changes for a note this device does not have wait here, one row per
/// field, the newer stamp winning.
pub(super) fn hold_step(tx: &Transaction<'_>, ch: &StepChange) -> Result<()> {
    tx.execute(
        "INSERT INTO sync_held_steps (note_id, field, value, hlc)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(note_id, field) DO UPDATE SET
             value = excluded.value,
             hlc = excluded.hlc
         WHERE excluded.hlc > sync_held_steps.hlc",
        params![
            ch.note_id,
            step_field_name(&ch.step_id, ch.part),
            serde_json::to_string(&ch.value)?,
            ch.hlc
        ],
    )?;
    Ok(())
}

/// Applies held step changes whose note now exists, once, and empties them.
/// The rows of one step are applied together, so its creation is whole.
pub(super) fn flush_held_steps(
    tx: &Transaction<'_>,
    now_ms: i64,
    report: &mut ApplyReport,
) -> Result<()> {
    let mut stmt = tx.prepare("SELECT note_id, field, value, hlc FROM sync_held_steps")?;
    let rows: Vec<(String, String, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);

    let mut by_note: BTreeMap<String, Vec<StepChange>> = BTreeMap::new();
    for (note_id, field, value, hlc) in rows {
        let Some((step_id, part)) = parse_step_field(&field) else {
            tx.execute(
                "DELETE FROM sync_held_steps WHERE note_id = ?1 AND field = ?2",
                params![note_id, field],
            )?;
            continue;
        };
        by_note
            .entry(note_id.clone())
            .or_default()
            .push(StepChange {
                note_id: note_id.clone(),
                step_id,
                part,
                value: serde_json::from_str(&value)?,
                hlc,
            });
    }

    for (note_id, changes) in by_note {
        if !note_exists(tx, &note_id)? {
            continue;
        }
        let refs: Vec<&StepChange> = changes.iter().collect();
        apply_steps_for_note(tx, &note_id, &refs, now_ms, report)?;
        tx.execute(
            "DELETE FROM sync_held_steps WHERE note_id = ?1",
            params![note_id],
        )?;
    }
    Ok(())
}
