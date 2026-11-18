//! Look storage: the user's looks in SQLite, the bundled ones from data.
//! Follows the `transfer.rs` pattern of keeping `impl Db` beside the feature
//! and using the connection accessors from `db.rs`. Documents are stored as
//! JSON text, and a save never rebuilds a stored document from typed
//! structs, so fields this build does not model survive every edit.

use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use crate::db::{now_ms, Db};
use crate::error::{CoreError, Result};
use crate::look::bundled::bundled;
use crate::look::code::decode_code;
use crate::look::derive::{resolve, ResolvedLook};
use crate::look::resolve;
use crate::look::{
    parse_stored, LookDoc, LookDocExt as _, ResolvedScene, SceneEdit, ThemeEdit, DEFAULT_LOOK_ID,
    MAX_USER_LOOKS,
};

pub use posthaste_model::LookSummary;

fn summary(id: String, doc: &LookDoc, builtin: bool) -> LookSummary {
    LookSummary {
        id,
        name: doc.name.clone(),
        author: doc.author.clone(),
        builtin,
        base: doc.theme.base,
        preview: doc.preview(),
        font_family: doc.theme.font_family.clone(),
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
            .map(|(id, doc)| summary(id, &doc, true))
            .collect();
        let mut stmt = self
            .conn()
            .prepare("SELECT id, doc FROM looks ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (id, json): (String, String) = row?;
            match parse_stored(&json) {
                Ok(doc) if doc.validate().is_ok() => out.push(summary(id, &doc, false)),
                Err(e) => log::warn!("stored look {id} unreadable, skipping: {e}"),
                Ok(_) => log::warn!("stored look {id} invalid, skipping"),
            }
        }
        Ok(out)
    }

    /// The document behind an id, bundled or stored. A stored look keeps
    /// every field this build does not model, so export can carry them.
    /// `NotFound` when the id is unknown or the stored row is unreadable.
    pub fn look_document(&self, id: &str) -> Result<LookDoc> {
        if is_builtin(id) {
            return bundled()
                .into_iter()
                .find(|(bid, _)| bid == id)
                .map(|(_, doc)| doc)
                .ok_or(CoreError::NotFound);
        }
        self.stored_document(id)?.ok_or(CoreError::NotFound)
    }

    /// The theme part of the look behind an id, as the editor edits it.
    /// `NotFound` when the id is unknown or the stored row is unreadable.
    pub fn look_theme(&self, id: &str) -> Result<ThemeEdit> {
        Ok(self.look_document(id)?.theme_edit())
    }

    /// The stored document behind a user id, keeping "missing" (`None`)
    /// separate from "unreadable" (an error), so a save can refuse to
    /// overwrite a row it cannot read instead of replacing it blindly.
    fn stored_document(&self, id: &str) -> Result<Option<LookDoc>> {
        let json: Option<String> = self
            .conn()
            .query_row("SELECT doc FROM looks WHERE id = ?1", [id], |r| r.get(0))
            .optional()?;
        match json {
            None => Ok(None),
            Some(json) => parse_stored(&json).map(Some),
        }
    }

    /// The scene of the look the user selected, ready to draw. Simple
    /// looks answers an empty scene, as does Plain: the setting is
    /// applied here, so no window can forget it.
    pub fn active_scene(&self) -> Result<ResolvedScene> {
        let settings = self.load_settings()?;
        if settings.simple_looks {
            return Ok(ResolvedScene::default());
        }
        let Some(id) = settings.look_id else {
            return Ok(ResolvedScene::default());
        };
        Ok(match self.look_document(&id) {
            Ok(doc) => resolve::resolve_scene(&doc),
            _ => ResolvedScene::default(),
        })
    }

    /// The scene and asset list of a look, as the editor edits them.
    pub fn look_scene(&self, id: &str) -> Result<SceneEdit> {
        let doc = self.look_document(id)?;
        Ok(SceneEdit {
            scene: doc.scene,
            assets: doc.assets,
        })
    }

    /// The look's full token map, validated and derived.
    pub fn resolve_look(&self, id: &str) -> Result<ResolvedLook> {
        let doc = self.look_document(id)?;
        resolve(id, &doc.theme_edit())
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

    /// Saves a new user look, or updates one by id. The edit is validated
    /// again here, so nothing reaches SQLite unchecked.
    ///
    /// The scene edit decides what happens to the look's scene and
    /// assets: `None` keeps them exactly as stored (a save from the
    /// colour editor, or from an editor whose scene has not loaded),
    /// `Some` replaces them with the edit, unknown layers included.
    pub fn save_look(
        &mut self,
        edit: ThemeEdit,
        scene: Option<SceneEdit>,
        id: Option<&str>,
    ) -> Result<LookSummary> {
        self.save_look_at(edit, scene, id, now_ms())
    }

    pub fn save_look_at(
        &mut self,
        edit: ThemeEdit,
        scene: Option<SceneEdit>,
        id: Option<&str>,
        now: i64,
    ) -> Result<LookSummary> {
        crate::look::validate_theme(&edit)?;
        // The editor only edits identity and theme. Keep the rest by
        // editing the stored document instead of rebuilding one — and if
        // the row exists but this build cannot read it, stop: writing a
        // fresh document over it would silently drop whatever it holds.
        let stored = match id {
            Some(id) if !is_builtin(id) => self.stored_document(id)?,
            _ => None,
        };
        let mut doc = match stored {
            Some(mut doc) => {
                doc.set_theme_from(&edit);
                doc
            }
            None => {
                let mut doc = new_document(&edit);
                doc.set_theme_from(&edit);
                doc
            }
        };
        if let Some(edit) = scene {
            doc.scene = edit.scene;
            doc.assets = edit.assets;
        }
        self.save_look_document_at(doc, id, now)
    }

    /// Saves a parsed document as (or over) a user look, keeping every
    /// part this build does not model. Import and the compatibility tests
    /// use this path; the editor uses `save_look`.
    pub fn save_look_document(&mut self, doc: LookDoc, id: Option<&str>) -> Result<LookSummary> {
        self.save_look_document_at(doc, id, now_ms())
    }

    fn save_look_document_at(
        &mut self,
        doc: LookDoc,
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

    /// Stores the look behind a share code as a new user look.
    pub fn import_look(&mut self, code: &str) -> Result<LookSummary> {
        let doc = decode_code(code)?;
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

/// A fresh document for an edit: envelope filled, theme from the edit.
fn new_document(edit: &ThemeEdit) -> LookDoc {
    LookDoc {
        format: crate::look::LOOK_FORMAT.into(),
        version: crate::look::LOOK_VERSION,
        name: String::new(),
        author: None,
        theme: posthaste_model::LookTheme {
            base: edit.base,
            colors: Default::default(),
            shape: Default::default(),
            font: Default::default(),
            font_family: None,
            extra: Default::default(),
        },
        scene: Default::default(),
        assets: Vec::new(),
        extra: Default::default(),
    }
}
