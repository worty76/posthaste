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
use crate::sync::record::record as sync_record;
use crate::sync::store::{next_seq, state_get, state_set, Field, STATE_CLOCK, STATE_DEVICE_ID};
use crate::{Db, Status};

pub const CONFLICT_SOURCE: &str = "sync-conflict";

/// One per-field change exchanged between devices.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Change {
    pub note_id: String,
    pub field: Field,
    pub value: Value,
    pub hlc: String,
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
    /// Reads up to `limit` recorded field changes with `seq > since_seq`,
    /// oldest first, returning the changes and the highest `seq` included.
    ///
    /// Changes that share the same `seq` are never split across pages unless
    /// `limit` is smaller than the number of fields on one write (at most 6).
    pub fn changes_since(&self, since_seq: i64, limit: usize) -> Result<(Vec<Change>, i64)> {
        if limit == 0 {
            return Ok((Vec::new(), since_seq));
        }
        let mut stmt = self.conn().prepare(
            "SELECT note_id, field, hlc, seq
             FROM sync_fields
             WHERE seq > ?1
             ORDER BY seq ASC, note_id ASC, field ASC
             LIMIT ?2",
        )?;
        let rows: Vec<(String, String, String, i64)> = stmt
            .query_map(params![since_seq, limit as i64], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(stmt);

        let mut changes = Vec::with_capacity(rows.len());
        let mut max_seq = since_seq;

        for (note_id, field_str, hlc, seq) in rows {
            max_seq = max_seq.max(seq);
            let Some(field) = Field::parse(&field_str) else {
                continue;
            };
            let value = match read_field_value(self.conn(), &note_id, field)? {
                Some(v) => v,
                None => continue,
            };
            changes.push(Change {
                note_id,
                field,
                value,
                hlc,
            });
        }

        Ok((changes, max_seq))
    }

    /// Merges a batch of incoming changes from `from_device` inside a single
    /// SQLite transaction. Idempotent: applying the same batch twice makes no
    /// changes on the second call.
    pub fn apply_changes(
        &mut self,
        from_device: &str,
        changes: &[Change],
        now_ms: i64,
    ) -> Result<ApplyReport> {
        let tx = self.conn_mut().transaction()?;
        let mut report = ApplyReport::default();
        if changes.is_empty() {
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
