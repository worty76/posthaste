//! Sync metadata storage: tracked fields, device identity, clock state, and
//! paired peers.

use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::sync::hlc::Hlc;
use crate::Db;

pub(crate) const STATE_DEVICE_ID: &str = "device_id";
pub(crate) const STATE_CLOCK: &str = "clock";
pub(crate) const STATE_NEXT_SEQ: &str = "next_seq";

/// Each note field tracked independently for last-writer-wins sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Field {
    Content,
    Details,
    Category,
    Status,
    Later,
    Deleted,
    Created,
    Purged,
}

impl Field {
    pub fn as_str(&self) -> &'static str {
        match self {
            Field::Content => "content",
            Field::Details => "details",
            Field::Category => "category",
            Field::Status => "status",
            Field::Later => "later",
            Field::Deleted => "deleted",
            Field::Created => "created",
            Field::Purged => "purged",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "content" => Some(Field::Content),
            "details" => Some(Field::Details),
            "category" => Some(Field::Category),
            "status" => Some(Field::Status),
            "later" => Some(Field::Later),
            "deleted" => Some(Field::Deleted),
            "created" => Some(Field::Created),
            "purged" => Some(Field::Purged),
            _ => None,
        }
    }
}

/// A paired device stored in `sync_peers`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Peer {
    pub device_id: String,
    pub name: String,
    pub addrs: Vec<String>,
    pub paired_at: i64,
    pub last_sync_at: Option<i64>,
    pub sent_up_to: i64,
    pub received_hlc: Option<String>,
    /// Lowest seq of step changes this peer has not been sent because it did
    /// not announce the steps capability. `None` when nothing is withheld.
    #[serde(default)]
    pub steps_resend_from: Option<i64>,
}

pub(crate) fn state_get(tx: &Transaction<'_>, key: &str) -> Result<Option<String>> {
    Ok(tx
        .query_row("SELECT value FROM sync_state WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .optional()?)
}

pub(crate) fn state_set(tx: &Transaction<'_>, key: &str, value: &str) -> Result<()> {
    tx.execute(
        "INSERT OR REPLACE INTO sync_state (key, value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}

pub(crate) fn next_seq(tx: &Transaction<'_>) -> Result<i64> {
    let seq: i64 = state_get(tx, STATE_NEXT_SEQ)?
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    state_set(tx, STATE_NEXT_SEQ, &(seq + 1).to_string())?;
    Ok(seq)
}

fn peer_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Peer> {
    let addrs_json: String = r.get(2)?;
    let addrs: Vec<String> = serde_json::from_str(&addrs_json).unwrap_or_default();
    Ok(Peer {
        device_id: r.get(0)?,
        name: r.get(1)?,
        addrs,
        paired_at: r.get(3)?,
        last_sync_at: r.get(4)?,
        sent_up_to: r.get(5)?,
        received_hlc: r.get(6)?,
        steps_resend_from: r.get(7)?,
    })
}

impl Db {
    /// Returns this device's sync id if sync has been initialised.
    pub fn sync_device_id(&self) -> Result<Option<String>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT value FROM sync_state WHERE key = ?1",
                [STATE_DEVICE_ID],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Sets this device's sync id once and backfills `sync_fields` for every
    /// note already in the database. Calling again when an id is already set
    /// is a no-op.
    pub fn set_sync_device_id(&mut self, id: &str) -> Result<()> {
        let tx = self.conn_mut().transaction()?;
        if state_get(&tx, STATE_DEVICE_ID)?.is_some() {
            return Ok(());
        }
        state_set(&tx, STATE_DEVICE_ID, id)?;

        // Backfill every existing note (including trashed notes) so peers
        // paired later receive the full current state.
        let mut stmt = tx.prepare(
            "SELECT id, updated_at, details, category, status, later_at, deleted_at
             FROM notes
             ORDER BY seq ASC",
        )?;
        struct BackfillRow {
            note_id: String,
            updated_at: i64,
            details: String,
            category: Option<String>,
            status: String,
            later_at: Option<i64>,
            deleted_at: Option<i64>,
        }
        let rows: Vec<BackfillRow> = stmt
            .query_map([], |r| {
                Ok(BackfillRow {
                    note_id: r.get(0)?,
                    updated_at: r.get(1)?,
                    details: r.get(2)?,
                    category: r.get(3)?,
                    status: r.get(4)?,
                    later_at: r.get(5)?,
                    deleted_at: r.get(6)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        drop(stmt);

        let mut last_clock: Option<Hlc> = None;
        for row in rows {
            let mut fields = vec![Field::Created, Field::Content];
            if !row.details.is_empty() {
                fields.push(Field::Details);
            }
            if row.category.is_some() {
                fields.push(Field::Category);
            }
            if row.status != "inbox" {
                fields.push(Field::Status);
            }
            if row.later_at.is_some() {
                fields.push(Field::Later);
            }
            if row.deleted_at.is_some() {
                fields.push(Field::Deleted);
            }

            let hlc = Hlc::tick(last_clock.as_ref(), row.updated_at, id);
            let hlc_str = hlc.format();
            for field in fields {
                let seq = next_seq(&tx)?;
                tx.execute(
                    "INSERT OR REPLACE INTO sync_fields (note_id, field, hlc, seq)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![row.note_id, field.as_str(), hlc_str, seq],
                )?;
            }
            last_clock = Some(hlc);
        }

        if let Some(clock) = last_clock {
            state_set(&tx, STATE_CLOCK, &clock.format())?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn is_paired(&self, device_id: &str) -> Result<bool> {
        let count: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM sync_peers WHERE device_id = ?1",
            [device_id],
            |r| r.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn list_peers(&self) -> Result<Vec<Peer>> {
        let mut stmt = self.conn().prepare(
            "SELECT device_id, name, addrs, paired_at, last_sync_at, sent_up_to, received_hlc,
                    steps_resend_from
             FROM sync_peers
             ORDER BY paired_at ASC",
        )?;
        let rows = stmt
            .query_map([], peer_from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_peer(&self, device_id: &str) -> Result<Option<Peer>> {
        self.conn()
            .query_row(
                "SELECT device_id, name, addrs, paired_at, last_sync_at, sent_up_to, received_hlc,
                        steps_resend_from
                 FROM sync_peers WHERE device_id = ?1",
                [device_id],
                peer_from_row,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn upsert_peer(
        &mut self,
        device_id: &str,
        name: &str,
        addrs: &[String],
        paired_at: i64,
    ) -> Result<Peer> {
        let addrs_json = serde_json::to_string(addrs)?;
        self.conn().execute(
            "INSERT INTO sync_peers (device_id, name, addrs, paired_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(device_id) DO UPDATE SET
                 name = excluded.name,
                 addrs = excluded.addrs",
            params![device_id, name, addrs_json, paired_at],
        )?;
        self.get_peer(device_id)?.ok_or(crate::CoreError::NotFound)
    }

    pub fn remove_peer(&mut self, device_id: &str) -> Result<()> {
        self.conn()
            .execute("DELETE FROM sync_peers WHERE device_id = ?1", [device_id])?;
        Ok(())
    }

    pub fn update_peer_sent_up_to(
        &mut self,
        device_id: &str,
        sent_up_to: i64,
        synced_at: i64,
    ) -> Result<()> {
        self.conn().execute(
            "UPDATE sync_peers
             SET sent_up_to = MAX(sent_up_to, ?2),
                 last_sync_at = ?3
             WHERE device_id = ?1",
            params![device_id, sent_up_to, synced_at],
        )?;
        Ok(())
    }

    pub fn update_peer_sync_time(&mut self, device_id: &str, synced_at: i64) -> Result<()> {
        self.conn().execute(
            "UPDATE sync_peers SET last_sync_at = ?2 WHERE device_id = ?1",
            params![device_id, synced_at],
        )?;
        Ok(())
    }

    /// Records the lowest seq of step changes withheld from this peer, so a
    /// later session can resend from there once it announces the capability.
    /// Keeping the lowest never loses an earlier marker.
    pub fn mark_steps_withheld(&self, device_id: &str, seq: i64) -> Result<()> {
        self.conn().execute(
            "UPDATE sync_peers SET steps_resend_from =
                 CASE WHEN steps_resend_from IS NULL OR ?2 < steps_resend_from
                      THEN ?2 ELSE steps_resend_from END
             WHERE device_id = ?1",
            params![device_id, seq],
        )?;
        Ok(())
    }

    /// Clears the withheld marker once every batch of the resend was
    /// acknowledged; merging is idempotent, so nothing is lost either way.
    pub fn clear_steps_resend_from(&self, device_id: &str) -> Result<()> {
        self.conn().execute(
            "UPDATE sync_peers SET steps_resend_from = NULL WHERE device_id = ?1",
            [device_id],
        )?;
        Ok(())
    }
}
