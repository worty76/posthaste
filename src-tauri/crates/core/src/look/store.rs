//! Look storage: the user's looks in SQLite, the bundled ones from data.
//! Follows the `transfer.rs` pattern of keeping `impl Db` beside the feature
//! and using the connection accessors from `db.rs`. Documents are stored as
//! JSON text in the format they were saved in; since this build, saves write
//! v2, and a save never rebuilds a stored document from typed structs, so
//! fields this build does not model survive every edit.

use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use crate::db::{now_ms, Db};
use crate::error::{CoreError, Result};
use crate::look::bundled::bundled;
use crate::look::code::decode_code;
use crate::look::derive::{resolve, ResolvedLook};
use crate::look::migrate::v1_to_v2;
use crate::look::{LookDocExt as _, LookDocument, DEFAULT_LOOK_ID, MAX_USER_LOOKS};

pub use posthaste_model::LookSummary;

fn summary(id: String, doc: &LookDocument, builtin: bool) -> LookSummary {
    let theme = doc.v1_view();
    LookSummary {
        id,
        name: theme.name.clone(),
        author: theme.author.clone(),
        builtin,
        base: theme.base,
        preview: theme.preview(),
    }
}

fn is_builtin(id: &str) -> bool {
    id.starts_with("builtin:")
}

impl Db {
    /// Bundled looks in gallery order, then the user's, newest first. A
    /// stored look that no longer parses or validates is skipped with a
    /// warning, so one bad row can never blank the gallery.
    pub fn list_looks(&self) -> Result<Vec<LookSummary>> {
        let mut out: Vec<LookSummary> = bundled()
            .into_iter()
            .map(|(id, doc)| summary(id, &LookDocument::V1(doc), true))
            .collect();
        let mut stmt = self
            .conn()
            .prepare("SELECT id, doc FROM looks ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (id, json): (String, String) = row?;
            match LookDocument::parse_stored(&json) {
                Ok(doc) if doc.validate().is_ok() => out.push(summary(id, &doc, false)),
                Err(e) => log::warn!("stored look {id} unreadable, skipping: {e}"),
                Ok(_) => log::warn!("stored look {id} invalid, skipping"),
            }
        }
        Ok(out)
    }

    /// The look document behind an id, bundled or stored, as the v1 view
    /// the editor edits. `NotFound` when the id is unknown or the stored
    /// row is unreadable.
    pub fn look_doc(&self, id: &str) -> Result<posthaste_model::LookDoc> {
        Ok(self.look_document(id)?.v1_view())
    }

    /// The document behind an id in the format it is stored in. A stored
    /// v2 look keeps every field this build does not model, so export can
    /// carry them; the v1 view is only a projection. `NotFound` when the
    /// id is unknown or the stored row is unreadable.
    pub fn look_document(&self, id: &str) -> Result<LookDocument> {
        if is_builtin(id) {
            return bundled()
                .into_iter()
                .find(|(bid, _)| bid == id)
                .map(|(_, doc)| LookDocument::V1(doc))
                .ok_or(CoreError::NotFound);
        }
        self.stored_document(id)?.ok_or(CoreError::NotFound)
    }

    /// The stored document behind a user id, keeping "missing" (`None`)
    /// separate from "unreadable" (an error), so a save can refuse to
    /// overwrite a row it cannot read instead of replacing it blindly.
    fn stored_document(&self, id: &str) -> Result<Option<LookDocument>> {
        let json: Option<String> = self
            .conn()
            .query_row("SELECT doc FROM looks WHERE id = ?1", [id], |r| r.get(0))
            .optional()?;
        match json {
            None => Ok(None),
            Some(json) => LookDocument::parse_stored(&json).map(Some),
        }
    }

    /// The look's full token map, validated and derived.
    pub fn resolve_look(&self, id: &str) -> Result<ResolvedLook> {
        let doc = self.look_doc(id)?;
        resolve(id, &doc)
    }

    /// The look the user selected, or `None` for Plain. A selection that no
    /// longer resolves (deleted, corrupt, removed from a later version)
    /// falls back to Ink, the factory default, so the app never loses its
    /// look by accident.
    pub fn active_look(&self) -> Result<Option<ResolvedLook>> {
        let Some(id) = self.load_settings()?.look_id else {
            return Ok(None);
        };
        Ok(self
            .resolve_look(&id)
            .or_else(|_| self.resolve_look(DEFAULT_LOOK_ID))
            .ok())
    }

    /// Saves a new user look, or updates one by id. The document is
    /// validated again here, so nothing reaches SQLite unchecked. The
    /// stored format is v2: a v1 document migrates on this save, and an
    /// update over a stored v2 look keeps the parts the v1 editor does
    /// not model.
    pub fn save_look(
        &mut self,
        doc: posthaste_model::LookDoc,
        id: Option<&str>,
    ) -> Result<LookSummary> {
        self.save_look_at(doc, id, now_ms())
    }

    pub fn save_look_at(
        &mut self,
        doc: posthaste_model::LookDoc,
        id: Option<&str>,
        now: i64,
    ) -> Result<LookSummary> {
        doc.validate()?;
        // The editor only edits identity and theme. Keep the rest by
        // editing the stored document instead of rebuilding one — and if
        // the row exists but this build cannot read it, stop: writing a
        // fresh document over it would silently drop whatever it holds.
        let stored = match id {
            Some(id) if !is_builtin(id) => self.stored_document(id)?,
            _ => None,
        };
        let to_store = match stored {
            Some(LookDocument::V2(mut v2)) => {
                v2.set_theme_from(&doc);
                LookDocument::V2(v2)
            }
            _ => LookDocument::V2(v1_to_v2(&doc)),
        };
        self.save_look_document_at(to_store, id, now)
    }

    /// Saves a parsed document of either format as (or over) a user look,
    /// keeping every part this build does not model. Import and the
    /// compatibility tests use this path; the editor uses `save_look`.
    pub fn save_look_document(
        &mut self,
        doc: LookDocument,
        id: Option<&str>,
    ) -> Result<LookSummary> {
        self.save_look_document_at(doc, id, now_ms())
    }

    fn save_look_document_at(
        &mut self,
        doc: LookDocument,
        id: Option<&str>,
        now: i64,
    ) -> Result<LookSummary> {
        doc.validate()?;
        let json = doc.to_json();
        match id {
            None => {
                let tx = self.conn_mut().transaction()?;
                // The count and the insert share one transaction, so two
                // windows saving at once cannot slip past the limit.
                let count: i64 = tx.query_row("SELECT COUNT(*) FROM looks", [], |r| r.get(0))?;
                if count as usize >= MAX_USER_LOOKS {
                    return Err(CoreError::TooManyLooks);
                }
                let new_id = format!("user:{}", Uuid::now_v7());
                tx.execute(
                    "INSERT INTO looks (id, doc, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
                    params![new_id, json, now],
                )?;
                tx.commit()?;
                Ok(summary(new_id, &doc, false))
            }
            Some(id) => {
                if is_builtin(id) {
                    return Err(CoreError::BuiltinLook);
                }
                let tx = self.conn_mut().transaction()?;
                let changed = tx.execute(
                    "UPDATE looks SET doc = ?1, updated_at = ?2 WHERE id = ?3",
                    params![json, now, id],
                )?;
                if changed == 0 {
                    return Err(CoreError::NotFound);
                }
                tx.commit()?;
                Ok(summary(id.to_string(), &doc, false))
            }
        }
    }

    /// Stores the look behind a share code as a new user look. A v1 code
    /// migrates to v2 here, so the stored copy is v2 from the first save.
    pub fn import_look(&mut self, code: &str) -> Result<LookSummary> {
        let doc = match decode_code(code)? {
            LookDocument::V1(v1) => LookDocument::V2(v1_to_v2(&v1)),
            v2 => v2,
        };
        self.save_look_document(doc, None)
    }

    /// Deletes a user look. Built-in looks are data, not rows, and stay.
    pub fn delete_look(&mut self, id: &str) -> Result<()> {
        if is_builtin(id) {
            return Err(CoreError::BuiltinLook);
        }
        let tx = self.conn_mut().transaction()?;
        let changed = tx.execute("DELETE FROM looks WHERE id = ?1", [id])?;
        tx.commit()?;
        if changed == 0 {
            return Err(CoreError::NotFound);
        }
        Ok(())
    }
}
