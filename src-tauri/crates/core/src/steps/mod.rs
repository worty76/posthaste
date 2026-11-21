//! Steps inside a note: storage, ranks, limits.
//!
//! A step belongs to a note and is written in the same transaction that
//! touches the note. Tests in `steps/tests.rs` (ranks and storage).

pub(crate) mod rank;

use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use crate::db::{now_ms, Db};
use crate::error::{CoreError, Result};
use crate::sync::merge::StepPart;
use crate::sync::record::record_step_parts;
use posthaste_model::{NewStep, Step, MAX_STEPS_PER_NOTE};

#[cfg(test)]
mod tests;

impl Db {
    /// The note's live steps, in display order: `rank`, then `id`.
    pub fn list_steps(&self, note_id: &str) -> Result<Vec<Step>> {
        let mut stmt = self.conn().prepare_cached(
            "SELECT id, note_id, text, done, rank FROM note_steps
             WHERE note_id = ?1 AND deleted_at IS NULL
             ORDER BY rank, id",
        )?;
        let rows = stmt.query_map([note_id], step_from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn add_steps(
        &mut self,
        note_id: &str,
        after: Option<&str>,
        lines: &[NewStep],
    ) -> Result<Vec<Step>> {
        self.add_steps_at(note_id, after, lines, now_ms())
    }

    /// Adds steps after `after` (`None` means first), all or nothing in one
    /// transaction. A paste that would pass the per-note limit is refused
    /// whole, so a refusal can never leave half of it behind.
    pub fn add_steps_at(
        &mut self,
        note_id: &str,
        after: Option<&str>,
        lines: &[NewStep],
        now: i64,
    ) -> Result<Vec<Step>> {
        let mut cleaned = Vec::with_capacity(lines.len());
        for line in lines {
            cleaned.push((clean_step_text(&line.text)?, line.done));
        }
        if cleaned.is_empty() {
            return Ok(Vec::new());
        }
        let tx = self.conn_mut().transaction()?;
        ensure_note_editable(&tx, note_id)?;
        let live: i64 = tx.query_row(
            "SELECT COUNT(*) FROM note_steps WHERE note_id = ?1 AND deleted_at IS NULL",
            [note_id],
            |r| r.get(0),
        )?;
        if live as usize + cleaned.len() > MAX_STEPS_PER_NOTE {
            return Err(CoreError::StepLimit {
                max: MAX_STEPS_PER_NOTE,
            });
        }
        // The step the block follows, and the first live step that will sit
        // after it: every new rank goes between the two.
        let after_rank = match after {
            Some(id) => Some(
                tx.query_row(
                    "SELECT rank FROM note_steps
                     WHERE id = ?1 AND note_id = ?2 AND deleted_at IS NULL",
                    params![id, note_id],
                    |r| r.get::<_, String>(0),
                )
                .optional()?
                .ok_or(CoreError::NotFound)?,
            ),
            None => None,
        };
        let next_rank = first_rank_after(&tx, note_id, after_rank.as_deref(), None)?;
        let mut added = Vec::with_capacity(cleaned.len());
        let mut before = after_rank;
        for (text, done) in cleaned {
            let rank = rank::rank_between(before.as_deref(), next_rank.as_deref());
            let step = Step {
                id: Uuid::now_v7().to_string(),
                note_id: note_id.to_owned(),
                text,
                done,
                rank: rank.clone(),
            };
            tx.execute(
                "INSERT INTO note_steps (id, note_id, text, done, rank, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                params![step.id, note_id, step.text, step.done as i64, rank, now],
            )?;
            record_step_parts(
                &tx,
                note_id,
                &step.id,
                &[StepPart::Text, StepPart::Done, StepPart::Rank],
                now,
            )?;
            before = Some(rank);
            added.push(step);
        }
        touch_note(&tx, note_id, now)?;
        tx.commit()?;
        Ok(added)
    }

    pub fn set_step_text(&mut self, id: &str, text: &str) -> Result<Step> {
        self.set_step_text_at(id, text, now_ms())
    }

    pub fn set_step_text_at(&mut self, id: &str, text: &str, now: i64) -> Result<Step> {
        let text = clean_step_text(text)?;
        let tx = self.conn_mut().transaction()?;
        let step = live_step(&tx, id)?;
        tx.execute(
            "UPDATE note_steps SET text = ?1, updated_at = ?2 WHERE id = ?3",
            params![text, now, id],
        )?;
        record_step_parts(&tx, &step.note_id, id, &[StepPart::Text], now)?;
        finish_step_write(&tx, &step.note_id, now)?;
        tx.commit()?;
        Ok(Step { text, ..step })
    }

    pub fn set_step_done(&mut self, id: &str, done: bool) -> Result<Step> {
        self.set_step_done_at(id, done, now_ms())
    }

    pub fn set_step_done_at(&mut self, id: &str, done: bool, now: i64) -> Result<Step> {
        let tx = self.conn_mut().transaction()?;
        let step = live_step(&tx, id)?;
        tx.execute(
            "UPDATE note_steps SET done = ?1, updated_at = ?2 WHERE id = ?3",
            params![done as i64, now, id],
        )?;
        record_step_parts(&tx, &step.note_id, id, &[StepPart::Done], now)?;
        finish_step_write(&tx, &step.note_id, now)?;
        tx.commit()?;
        Ok(Step { done, ..step })
    }

    pub fn move_step(&mut self, id: &str, after: Option<&str>) -> Result<Step> {
        self.move_step_at(id, after, now_ms())
    }

    /// Moves a step to sit directly after `after` (`None` means first). Only
    /// the rank changes; the id stays, so nothing else about the step moves.
    pub fn move_step_at(&mut self, id: &str, after: Option<&str>, now: i64) -> Result<Step> {
        let tx = self.conn_mut().transaction()?;
        let step = live_step(&tx, id)?;
        if after == Some(id) {
            return Ok(step);
        }
        let after_rank = match after {
            Some(other) => Some(
                tx.query_row(
                    "SELECT rank FROM note_steps
                     WHERE id = ?1 AND note_id = ?2 AND deleted_at IS NULL",
                    params![other, step.note_id],
                    |r| r.get::<_, String>(0),
                )
                .optional()?
                .ok_or(CoreError::NotFound)?,
            ),
            None => None,
        };
        let next_rank = first_rank_after(&tx, &step.note_id, after_rank.as_deref(), Some(id))?;
        let rank = rank::rank_between(after_rank.as_deref(), next_rank.as_deref());
        tx.execute(
            "UPDATE note_steps SET rank = ?1, updated_at = ?2 WHERE id = ?3",
            params![rank, now, id],
        )?;
        record_step_parts(&tx, &step.note_id, id, &[StepPart::Rank], now)?;
        finish_step_write(&tx, &step.note_id, now)?;
        tx.commit()?;
        Ok(Step { rank, ..step })
    }

    pub fn remove_step(&mut self, id: &str) -> Result<()> {
        self.remove_step_at(id, now_ms())
    }

    /// Removes a step by leaving a tombstone behind, so other devices learn
    /// of the removal and an undo can bring the step back.
    pub fn remove_step_at(&mut self, id: &str, now: i64) -> Result<()> {
        let tx = self.conn_mut().transaction()?;
        let step = live_step(&tx, id)?;
        tx.execute(
            "UPDATE note_steps SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        record_step_parts(&tx, &step.note_id, id, &[StepPart::Deleted], now)?;
        finish_step_write(&tx, &step.note_id, now)?;
        tx.commit()?;
        Ok(())
    }

    pub fn restore_step(&mut self, id: &str) -> Result<Step> {
        self.restore_step_at(id, now_ms())
    }

    /// Brings a removed step back, unchanged. A newer explicit undelete is
    /// the only thing that clears a tombstone.
    pub fn restore_step_at(&mut self, id: &str, now: i64) -> Result<Step> {
        let tx = self.conn_mut().transaction()?;
        let changed = tx.execute(
            "UPDATE note_steps SET deleted_at = NULL, updated_at = ?1 WHERE id = ?2
             AND deleted_at IS NOT NULL",
            params![now, id],
        )?;
        if changed == 0 {
            return Err(CoreError::NotFound);
        }
        let step = live_step(&tx, id)?;
        record_step_parts(&tx, &step.note_id, id, &[StepPart::Deleted], now)?;
        finish_step_write(&tx, &step.note_id, now)?;
        tx.commit()?;
        Ok(step)
    }
}

/// A step that exists and is not tombstoned, with its note's id.
fn live_step(tx: &rusqlite::Transaction<'_>, id: &str) -> Result<Step> {
    tx.query_row(
        "SELECT s.id, s.note_id, s.text, s.done, s.rank FROM note_steps s
         WHERE s.id = ?1 AND s.deleted_at IS NULL",
        [id],
        step_from_row,
    )
    .optional()?
    .ok_or(CoreError::NotFound)
}

/// The rank of the first live step that sorts after the position
/// `after_rank`, skipping `except` (the step being moved). `after_rank` of
/// `None` means the very front.
fn first_rank_after(
    tx: &rusqlite::Transaction<'_>,
    note_id: &str,
    after_rank: Option<&str>,
    except: Option<&str>,
) -> Result<Option<String>> {
    let (low, low_id) = (after_rank.unwrap_or(""), "");
    tx.query_row(
        "SELECT rank FROM note_steps
         WHERE note_id = ?1 AND deleted_at IS NULL AND (?2 IS NULL OR id != ?2)
           AND (rank > ?3 OR (rank = ?3 AND id > ?4))
         ORDER BY rank, id LIMIT 1",
        params![note_id, except, low, low_id],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .map_err(Into::into)
}

/// The note the step belongs to must still be open, and its clock moves.
fn finish_step_write(tx: &rusqlite::Transaction<'_>, note_id: &str, now: i64) -> Result<()> {
    ensure_note_editable(tx, note_id)?;
    touch_note(tx, note_id, now)
}

/// `NotFound` for an unknown note, `NoteTrashed` for one in the trash.
fn ensure_note_editable(tx: &rusqlite::Transaction<'_>, note_id: &str) -> Result<()> {
    let trashed: Option<bool> = tx
        .query_row(
            "SELECT deleted_at IS NOT NULL FROM notes WHERE id = ?1",
            [note_id],
            |r| r.get(0),
        )
        .optional()?;
    match trashed {
        None => Err(CoreError::NotFound),
        Some(true) => Err(CoreError::NoteTrashed),
        Some(false) => Ok(()),
    }
}

fn touch_note(tx: &rusqlite::Transaction<'_>, note_id: &str, now: i64) -> Result<()> {
    tx.execute(
        "UPDATE notes SET updated_at = ?1 WHERE id = ?2",
        params![now, note_id],
    )?;
    Ok(())
}

fn step_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Step> {
    Ok(Step {
        id: r.get(0)?,
        note_id: r.get(1)?,
        text: r.get(2)?,
        done: r.get::<_, i64>(3)? != 0,
        rank: r.get(4)?,
    })
}

/// Steps are one trimmed line of bounded length; anything else is refused,
/// never cut, so what the person typed is never quietly changed.
fn clean_step_text(text: &str) -> Result<String> {
    let text = text.trim();
    if text.is_empty()
        || text.chars().count() > posthaste_model::MAX_STEP_CHARS
        || text.chars().any(|c| c == '\n' || c == '\r')
    {
        return Err(CoreError::StepInvalid);
    }
    Ok(text.to_owned())
}
