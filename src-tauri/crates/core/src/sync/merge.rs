//! Exporting local changes since a sequence number and merging incoming
//! per-field changes from another device.
//!
//! Never loses user text: when two devices concurrently edit `content` or
//! `details`, or when an incoming purge deletes a note with a newer local text
//! edit, the losing text is preserved as a new inbox note with
//! `source = 'sync-conflict'`.

use std::collections::BTreeMap;

use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::{clean_category, clean_content};
use crate::details::clean_details;
use crate::error::Result;
use crate::sync::hlc::Hlc;
use crate::sync::record::{record as sync_record, step_field_name};
use crate::sync::store::{next_seq, state_get, state_set, Field, STATE_CLOCK, STATE_DEVICE_ID};
use crate::{Db, Status};
use posthaste_model::MAX_STEP_CHARS;

pub const CONFLICT_SOURCE: &str = "sync-conflict";

/// One per-field change exchanged between devices.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Change {
    pub note_id: String,
    pub field: Field,
    pub value: Value,
    pub hlc: String,
}

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

/// What one export pass produced. `up_to` only ever covers changes the
/// caller will actually send; `withheld_from` is the lowest seq among step
/// changes the caller withholds, so they can be resent later.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Changes {
    pub changes: Vec<Change>,
    pub steps: Vec<StepChange>,
    pub up_to: i64,
    pub withheld_from: Option<i64>,
}

/// Summary of applying one batch of incoming changes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyReport {
    pub applied: usize,
    pub skipped: usize,
    pub conflicts: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StatusValue {
    status: String,
    completed_at: Option<i64>,
    archived_at: Option<i64>,
}

impl Db {
    /// Reads recorded field changes with `seq > since_seq`, oldest first:
    /// note changes and step changes, with the highest `seq` covered.
    ///
    /// Rows that share one `seq` are one write and are never split across
    /// pages, so a receiver never sees half of one write. With
    /// `include_steps` the step rows are exported; without it they are
    /// reported in `withheld_from` (their lowest seq) and `up_to` stays
    /// below them, so a peer without the capability is never moved past
    /// changes it cannot receive.
    pub fn changes_since(
        &self,
        since_seq: i64,
        limit: usize,
        include_steps: bool,
    ) -> Result<Changes> {
        if limit == 0 {
            return Ok(Changes {
                up_to: since_seq,
                ..Changes::default()
            });
        }
        // A write touches at most 6 note fields; fetching past the limit and
        // trimming back to a whole write keeps pages cut at seq boundaries.
        let mut stmt = self.conn().prepare(
            "SELECT note_id, field, hlc, seq
             FROM sync_fields
             WHERE seq > ?1
             ORDER BY seq ASC, note_id ASC, field ASC
             LIMIT ?2",
        )?;
        let rows: Vec<(String, String, String, i64)> = stmt
            .query_map(params![since_seq, limit as i64 + 6], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(stmt);

        // The number of rows to take: everything, or back to the last whole
        // write within `limit` rows.
        let mut cut = rows.len();
        if rows.len() > limit {
            let last_full = rows[..limit]
                .iter()
                .rev()
                .position(|(_, _, _, seq)| *seq != rows[limit].3)
                .map(|back| limit - back)
                .unwrap_or(0);
            cut = last_full;
        }

        let mut out = Changes::default();
        let mut withheld = i64::MAX;
        for (index, (note_id, field_str, hlc, seq)) in rows.into_iter().enumerate() {
            if index == cut {
                break;
            }
            if let Some(field) = Field::parse(&field_str) {
                let value = match read_field_value(self.conn(), &note_id, field)? {
                    Some(v) => v,
                    None => continue,
                };
                out.changes.push(Change {
                    note_id,
                    field,
                    value,
                    hlc,
                });
                out.up_to = out.up_to.max(seq);
            } else if let Some((step_id, part)) = parse_step_field(&field_str) {
                if include_steps {
                    let Some(value) = read_step_value(self.conn(), &step_id, part)? else {
                        continue;
                    };
                    out.steps.push(StepChange {
                        note_id,
                        step_id,
                        part,
                        value,
                        hlc,
                    });
                    out.up_to = out.up_to.max(seq);
                } else {
                    withheld = withheld.min(seq);
                }
            }
        }
        if withheld != i64::MAX {
            out.withheld_from = Some(withheld);
        }
        Ok(out)
    }

    /// Merges a batch of incoming changes from `from_device` inside a single
    /// SQLite transaction. Idempotent: applying the same batch twice makes no
    /// changes on the second call.
    pub fn apply_changes(
        &mut self,
        from_device: &str,
        changes: &[Change],
        steps: &[StepChange],
        now_ms: i64,
    ) -> Result<ApplyReport> {
        let tx = self.conn_mut().transaction()?;
        let mut report = ApplyReport::default();
        if changes.is_empty() && steps.is_empty() {
            return Ok(report);
        }

        let local_device = state_get(&tx, STATE_DEVICE_ID)?.unwrap_or_default();
        let mut clock = state_get(&tx, STATE_CLOCK)?.and_then(|s| Hlc::parse(&s));

        // Peer watermarks for detecting concurrent edits since last sync.
        let (peer_sent_up_to, peer_received_hlc): (i64, Option<String>) = tx
            .query_row(
                "SELECT sent_up_to, received_hlc FROM sync_peers WHERE device_id = ?1",
                [from_device],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
            .unwrap_or((0, None));

        // Group by note_id so unknown notes can be inserted in a valid order
        // (created -> content -> remaining fields).
        let mut by_note: BTreeMap<&str, Vec<&Change>> = BTreeMap::new();
        for ch in changes {
            by_note.entry(ch.note_id.as_str()).or_default().push(ch);
        }

        let mut max_received_hlc = peer_received_hlc.clone();

        for (note_id, mut note_changes) in by_note {
            note_changes.sort_by_key(|c| field_apply_order(c.field));

            // Advance the peer's received HLC watermark as we see incoming stamps.
            for ch in &note_changes {
                if max_received_hlc
                    .as_deref()
                    .map(|prev| ch.hlc.as_str() > prev)
                    .unwrap_or(true)
                {
                    max_received_hlc = Some(ch.hlc.clone());
                }
                if let Some(incoming_hlc) = Hlc::parse(&ch.hlc) {
                    clock = Some(Hlc::observe(
                        clock.as_ref(),
                        &incoming_hlc,
                        now_ms,
                        &local_device,
                    ));
                }
            }

            // If this note is already purged locally, ignore everything.
            if field_hlc(&tx, note_id, Field::Purged)?.is_some() {
                report.skipped += note_changes.len();
                continue;
            }

            // If the incoming batch purges this note, handle purge first.
            if let Some(purge_ch) = note_changes.iter().find(|c| c.field == Field::Purged) {
                let saved_conflict =
                    maybe_save_purge_conflict(&tx, note_id, peer_received_hlc.as_deref(), now_ms)?;
                if saved_conflict {
                    report.conflicts += 1;
                }
                tx.execute("DELETE FROM notes WHERE id = ?1", [note_id])?;
                tx.execute("DELETE FROM sync_held_steps WHERE note_id = ?1", [note_id])?;
                let seq = next_seq(&tx)?;
                tx.execute(
                    "INSERT INTO sync_fields (note_id, field, hlc, seq)
                     VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT(note_id, field) DO UPDATE SET
                         hlc = excluded.hlc,
                         seq = excluded.seq",
                    params![note_id, Field::Purged.as_str(), purge_ch.hlc, seq],
                )?;
                report.applied += 1;
                report.skipped += note_changes.len().saturating_sub(1);
                continue;
            }

            // Ensure the note row exists if this is a brand-new note from the peer.
            let mut exists = note_exists(&tx, note_id)?;
            if !exists {
                let created_val = note_changes
                    .iter()
                    .find(|c| c.field == Field::Created)
                    .and_then(|c| {
                        c.value
                            .as_i64()
                            .or_else(|| c.value.get("created_at").and_then(|v| v.as_i64()))
                    });
                let source_val = note_changes
                    .iter()
                    .find(|c| c.field == Field::Created)
                    .and_then(|c| c.value.get("source").and_then(|v| v.as_str()));
                let content_ch = note_changes.iter().find(|c| c.field == Field::Content);
                if let Some(content_ch) = content_ch {
                    if let Some(raw) = content_ch.value.as_str() {
                        if let Ok(cleaned) = clean_content(raw) {
                            let created_at = created_val.unwrap_or(now_ms);
                            tx.execute(
                                "INSERT INTO notes (id, content, status, source, created_at, updated_at)
                                 VALUES (?1, ?2, 'inbox', ?3, ?4, ?4)",
                                params![note_id, cleaned, source_val, created_at],
                            )?;
                            exists = true;
                        }
                    }
                }
            }

            if !exists {
                report.skipped += note_changes.len();
                continue;
            }

            for ch in note_changes {
                let local_meta = field_meta(&tx, note_id, ch.field)?;
                let local_wins = local_meta
                    .as_ref()
                    .is_some_and(|(local_hlc, _)| local_hlc.as_str() >= ch.hlc.as_str());

                if local_wins {
                    // Even when the local side wins LWW, check if this is a
                    // concurrent text edit from the peer so the peer's losing
                    // text is not silently discarded.
                    if let Some((local_hlc, local_seq)) = &local_meta {
                        if local_hlc != &ch.hlc
                            && *local_seq > peer_sent_up_to
                            && matches!(ch.field, Field::Content | Field::Details)
                            && save_losing_incoming_text(&tx, note_id, ch, now_ms)?
                        {
                            report.conflicts += 1;
                        }
                    }
                    report.skipped += 1;
                    continue;
                }

                // Incoming wins LWW. Before overwriting `content` or `details`,
                // check if the local side edited it concurrently since the last
                // sync with this peer.
                if let Some((_, local_seq)) = &local_meta {
                    if *local_seq > peer_sent_up_to
                        && matches!(ch.field, Field::Content | Field::Details)
                        && save_losing_local_text(&tx, note_id, ch, now_ms)?
                    {
                        report.conflicts += 1;
                    }
                }

                if apply_single_field(&tx, note_id, ch, now_ms)? {
                    let seq = next_seq(&tx)?;
                    tx.execute(
                        "INSERT INTO sync_fields (note_id, field, hlc, seq)
                         VALUES (?1, ?2, ?3, ?4)
                         ON CONFLICT(note_id, field) DO UPDATE SET
                             hlc = excluded.hlc,
                             seq = excluded.seq",
                        params![note_id, ch.field.as_str(), ch.hlc, seq],
                    )?;
                    report.applied += 1;
                } else {
                    report.skipped += 1;
                }
            }
        }

        // Steps travel with the notes they belong to; changes for a note this
        // device does not have yet wait in the holding table.
        for (note_id, step_changes) in by_step_note(steps) {
            observe_step_clocks(
                &mut clock,
                &mut max_received_hlc,
                &step_changes,
                &local_device,
                now_ms,
            );
            if field_hlc(&tx, note_id, Field::Purged)?.is_some() {
                report.skipped += step_changes.len();
                continue;
            }
            if !note_exists(&tx, note_id)? {
                for ch in &step_changes {
                    hold_step(&tx, ch)?;
                }
                report.skipped += step_changes.len();
                continue;
            }
            apply_steps_for_note(&tx, note_id, &step_changes, now_ms, &mut report)?;
        }
        // A note that arrived in this batch may complete earlier held steps.
        flush_held_steps(&tx, now_ms, &mut report)?;

        if let Some(c) = clock {
            state_set(&tx, STATE_CLOCK, &c.format())?;
        }
        if let Some(hlc) = max_received_hlc {
            tx.execute(
                "UPDATE sync_peers
                 SET received_hlc = CASE
                     WHEN received_hlc IS NULL OR received_hlc < ?2 THEN ?2
                     ELSE received_hlc
                 END
                 WHERE device_id = ?1",
                params![from_device, hlc],
            )?;
        }

        tx.commit()?;
        Ok(report)
    }
}

fn field_apply_order(field: Field) -> u8 {
    match field {
        Field::Created => 0,
        Field::Content => 1,
        Field::Details => 2,
        Field::Category => 3,
        Field::Status => 4,
        Field::Later => 5,
        Field::Deleted => 6,
        Field::Purged => 7,
    }
}

/// Groups step changes by the note they belong to, keeping their order.
fn by_step_note(steps: &[StepChange]) -> BTreeMap<&str, Vec<&StepChange>> {
    let mut by_note: BTreeMap<&str, Vec<&StepChange>> = BTreeMap::new();
    for ch in steps {
        by_note.entry(ch.note_id.as_str()).or_default().push(ch);
    }
    by_note
}

/// Incoming step stamps advance the clock and the peer watermark exactly
/// like note stamps do.
fn observe_step_clocks(
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
fn read_step_value(
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
fn apply_steps_for_note(
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
fn hold_step(tx: &Transaction<'_>, ch: &StepChange) -> Result<()> {
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
fn flush_held_steps(tx: &Transaction<'_>, now_ms: i64, report: &mut ApplyReport) -> Result<()> {
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

struct NoteRow {
    content: String,
    details: String,
    category: Option<String>,
    status: String,
    completed_at: Option<i64>,
    archived_at: Option<i64>,
    later_at: Option<i64>,
    deleted_at: Option<i64>,
    created_at: i64,
    source: Option<String>,
}

fn read_field_value(
    conn: &rusqlite::Connection,
    note_id: &str,
    field: Field,
) -> Result<Option<Value>> {
    if field == Field::Purged {
        return Ok(Some(Value::Null));
    }
    let row: Option<NoteRow> = conn
        .query_row(
            "SELECT content, details, category, status, completed_at, archived_at,
                    later_at, deleted_at, created_at, source
             FROM notes WHERE id = ?1",
            [note_id],
            |r| {
                Ok(NoteRow {
                    content: r.get(0)?,
                    details: r.get(1)?,
                    category: r.get(2)?,
                    status: r.get(3)?,
                    completed_at: r.get(4)?,
                    archived_at: r.get(5)?,
                    later_at: r.get(6)?,
                    deleted_at: r.get(7)?,
                    created_at: r.get(8)?,
                    source: r.get(9)?,
                })
            },
        )
        .optional()?;

    let Some(row) = row else {
        return Ok(None);
    };

    let val = match field {
        Field::Content => json!(row.content),
        Field::Details => json!(row.details),
        Field::Category => json!(row.category),
        Field::Status => json!(StatusValue {
            status: row.status,
            completed_at: row.completed_at,
            archived_at: row.archived_at,
        }),
        Field::Later => json!(row.later_at),
        Field::Deleted => json!(row.deleted_at),
        Field::Created => {
            if let Some(src) = row.source {
                json!({ "created_at": row.created_at, "source": src })
            } else {
                json!(row.created_at)
            }
        }
        Field::Purged => Value::Null,
    };
    Ok(Some(val))
}

fn note_exists(tx: &Transaction<'_>, note_id: &str) -> Result<bool> {
    let count: i64 = tx.query_row("SELECT COUNT(*) FROM notes WHERE id = ?1", [note_id], |r| {
        r.get(0)
    })?;
    Ok(count > 0)
}

fn field_hlc(tx: &Transaction<'_>, note_id: &str, field: Field) -> Result<Option<String>> {
    Ok(tx
        .query_row(
            "SELECT hlc FROM sync_fields WHERE note_id = ?1 AND field = ?2",
            params![note_id, field.as_str()],
            |r| r.get(0),
        )
        .optional()?)
}

fn field_meta(tx: &Transaction<'_>, note_id: &str, field: Field) -> Result<Option<(String, i64)>> {
    Ok(tx
        .query_row(
            "SELECT hlc, seq FROM sync_fields WHERE note_id = ?1 AND field = ?2",
            params![note_id, field.as_str()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?)
}

fn apply_single_field(
    tx: &Transaction<'_>,
    note_id: &str,
    ch: &Change,
    now_ms: i64,
) -> Result<bool> {
    let updated_at = Hlc::parse(&ch.hlc).map(|h| h.wall_ms).unwrap_or(now_ms);
    match ch.field {
        Field::Created => {
            let Some(created) = ch
                .value
                .as_i64()
                .or_else(|| ch.value.get("created_at").and_then(|v| v.as_i64()))
            else {
                return Ok(false);
            };
            let source = ch.value.get("source").and_then(|v| v.as_str());
            tx.execute(
                "UPDATE notes SET created_at = ?1, source = COALESCE(?2, source) WHERE id = ?3",
                params![created, source, note_id],
            )?;
            Ok(true)
        }
        Field::Content => {
            let Some(raw) = ch.value.as_str() else {
                return Ok(false);
            };
            let Ok(content) = clean_content(raw) else {
                return Ok(false);
            };
            tx.execute(
                "UPDATE notes SET content = ?1, updated_at = MAX(updated_at, ?2) WHERE id = ?3",
                params![content, updated_at, note_id],
            )?;
            Ok(true)
        }
        Field::Details => {
            let Some(raw) = ch.value.as_str() else {
                return Ok(false);
            };
            let Ok(details) = clean_details(raw, true) else {
                return Ok(false);
            };
            tx.execute(
                "UPDATE notes SET details = ?1, updated_at = MAX(updated_at, ?2) WHERE id = ?3",
                params![details, updated_at, note_id],
            )?;
            Ok(true)
        }
        Field::Category => {
            let category = ch.value.as_str().and_then(|s| clean_category(Some(s)));
            tx.execute(
                "UPDATE notes SET category = ?1, updated_at = MAX(updated_at, ?2) WHERE id = ?3",
                params![category, updated_at, note_id],
            )?;
            Ok(true)
        }
        Field::Status => {
            let Ok(sv) = serde_json::from_value::<StatusValue>(ch.value.clone()) else {
                return Ok(false);
            };
            let Some(status) = Status::parse(&sv.status) else {
                return Ok(false);
            };
            tx.execute(
                "UPDATE notes
                 SET status = ?1, completed_at = ?2, archived_at = ?3,
                     updated_at = MAX(updated_at, ?4)
                 WHERE id = ?5",
                params![
                    status.as_str(),
                    sv.completed_at,
                    sv.archived_at,
                    updated_at,
                    note_id
                ],
            )?;
            Ok(true)
        }
        Field::Later => {
            let later_at = ch.value.as_i64();
            tx.execute(
                "UPDATE notes SET later_at = ?1, updated_at = MAX(updated_at, ?2) WHERE id = ?3",
                params![later_at, updated_at, note_id],
            )?;
            Ok(true)
        }
        Field::Deleted => {
            let deleted_at = ch.value.as_i64();
            tx.execute(
                "UPDATE notes SET deleted_at = ?1 WHERE id = ?2",
                params![deleted_at, note_id],
            )?;
            Ok(true)
        }
        Field::Purged => Ok(false),
    }
}

/// Inserts a `sync-conflict` inbox note and records it in `sync_fields` so the
/// preserved text is also visible and synced without losing anything.
fn insert_conflict_note(tx: &Transaction<'_>, content: &str, now_ms: i64) -> Result<bool> {
    let Ok(cleaned) = clean_content(content) else {
        return Ok(false);
    };
    let id = Uuid::now_v7().to_string();
    tx.execute(
        "INSERT INTO notes (id, content, status, source, created_at, updated_at)
         VALUES (?1, ?2, 'inbox', ?3, ?4, ?4)",
        params![id, cleaned, CONFLICT_SOURCE, now_ms],
    )?;
    sync_record(tx, &id, &[Field::Created, Field::Content], now_ms)?;
    Ok(true)
}

fn save_losing_local_text(
    tx: &Transaction<'_>,
    note_id: &str,
    incoming: &Change,
    now_ms: i64,
) -> Result<bool> {
    let row: Option<(String, String)> = tx
        .query_row(
            "SELECT content, details FROM notes WHERE id = ?1",
            [note_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((local_content, local_details)) = row else {
        return Ok(false);
    };
    let incoming_str = incoming.value.as_str().unwrap_or("");
    match incoming.field {
        Field::Content => {
            if local_content == incoming_str {
                return Ok(false);
            }
            insert_conflict_note(tx, &local_content, now_ms)
        }
        Field::Details => {
            if local_details == incoming_str || local_details.trim().is_empty() {
                return Ok(false);
            }
            let title = local_content.lines().next().unwrap_or(&local_content);
            let combined = format!("{title}\n{local_details}");
            insert_conflict_note(tx, &combined, now_ms)
        }
        _ => Ok(false),
    }
}

fn save_losing_incoming_text(
    tx: &Transaction<'_>,
    note_id: &str,
    incoming: &Change,
    now_ms: i64,
) -> Result<bool> {
    let row: Option<(String, String)> = tx
        .query_row(
            "SELECT content, details FROM notes WHERE id = ?1",
            [note_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((local_content, local_details)) = row else {
        return Ok(false);
    };
    let incoming_str = incoming.value.as_str().unwrap_or("");
    match incoming.field {
        Field::Content => {
            if local_content == incoming_str {
                return Ok(false);
            }
            insert_conflict_note(tx, incoming_str, now_ms)
        }
        Field::Details => {
            if local_details == incoming_str || incoming_str.trim().is_empty() {
                return Ok(false);
            }
            let title = local_content.lines().next().unwrap_or(&local_content);
            let combined = format!("{title}\n{incoming_str}");
            insert_conflict_note(tx, &combined, now_ms)
        }
        _ => Ok(false),
    }
}

fn maybe_save_purge_conflict(
    tx: &Transaction<'_>,
    note_id: &str,
    peer_received_hlc: Option<&str>,
    now_ms: i64,
) -> Result<bool> {
    let row: Option<(String, String)> = tx
        .query_row(
            "SELECT content, details FROM notes WHERE id = ?1",
            [note_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((local_content, local_details)) = row else {
        return Ok(false);
    };

    let is_newer = |hlc: Option<String>| -> bool {
        match (hlc, peer_received_hlc) {
            (Some(h), Some(prev)) => h.as_str() > prev,
            (Some(_), None) => true,
            _ => false,
        }
    };

    let content_newer = is_newer(field_hlc(tx, note_id, Field::Content)?);
    let details_newer = is_newer(field_hlc(tx, note_id, Field::Details)?);

    if content_newer || details_newer {
        let text = if !local_details.trim().is_empty() && details_newer {
            let title = local_content.lines().next().unwrap_or(&local_content);
            format!("{title}\n{local_details}")
        } else {
            local_content
        };
        return insert_conflict_note(tx, &text, now_ms);
    }
    Ok(false)
}
