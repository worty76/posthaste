//! Note details: the description and the files attached to a note.

use rusqlite::{params, OptionalExtension, Row};
use uuid::Uuid;

use crate::db::{now_ms, Db};
use crate::error::{CoreError, Result};
use posthaste_model::*;

const MAX_NAME_CHARS: usize = 120;
const ATTACHMENT_COLUMNS: &str = "a.id, a.note_id, a.name, a.mime, a.size, a.created_at";

impl Db {
    /// The note with its description and the list of its attachments (not
    /// their bytes).
    pub fn note_detail(&self, id: &str) -> Result<NoteDetail> {
        let note = self.get_note(id)?;
        let details: String =
            self.conn()
                .query_row("SELECT details FROM notes WHERE id = ?1", [id], |r| {
                    r.get(0)
                })?;
        let mut stmt = self.conn().prepare_cached(&format!(
            "SELECT {ATTACHMENT_COLUMNS} FROM attachments a
             WHERE a.note_id = ?1 ORDER BY a.created_at, a.id"
        ))?;
        let attachments = stmt
            .query_map([id], attachment_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(NoteDetail {
            note,
            details,
            attachments,
        })
    }

    pub fn set_details(&mut self, id: &str, details: &str) -> Result<Note> {
        self.set_details_at(id, details, now_ms())
    }

    /// Replaces the description. Empty text clears it. Saved as typed apart
    /// from line endings, because this runs while the user is still typing.
    pub fn set_details_at(&mut self, id: &str, details: &str, now: i64) -> Result<Note> {
        let details = clean_details(details, false)?;
        let changed = self.conn().execute(
            "UPDATE notes SET details = ?1, updated_at = ?2 WHERE id = ?3 AND deleted_at IS NULL",
            params![details, now, id],
        )?;
        if changed == 0 {
            return Err(CoreError::NotFound);
        }
        self.get_note(id)
    }

    pub fn add_attachment(
        &mut self,
        note_id: &str,
        name: &str,
        mime: &str,
        bytes: &[u8],
    ) -> Result<Attachment> {
        self.add_attachment_at(note_id, name, mime, bytes, now_ms())
    }

    /// Stores a file on a note. `mime` is a hint (a pasted image knows its
    /// type); the file name's extension wins when it is recognised.
    pub fn add_attachment_at(
        &mut self,
        note_id: &str,
        name: &str,
        mime: &str,
        bytes: &[u8],
        now: i64,
    ) -> Result<Attachment> {
        let name = clean_file_name(name);
        if bytes.is_empty() {
            return Err(CoreError::AttachmentEmpty);
        }
        if bytes.len() > MAX_ATTACHMENT_BYTES {
            return Err(CoreError::AttachmentTooLarge {
                size_mb: format!("{:.1}", bytes.len() as f64 / (1024.0 * 1024.0)),
            });
        }
        let tx = self.conn_mut().transaction()?;
        let live: Option<i64> = tx
            .query_row(
                "SELECT (SELECT COUNT(*) FROM attachments WHERE note_id = ?1)
                 FROM notes WHERE id = ?1 AND deleted_at IS NULL",
                [note_id],
                |r| r.get(0),
            )
            .optional()?;
        let Some(count) = live else {
            return Err(CoreError::NotFound);
        };
        if count as usize >= MAX_ATTACHMENTS_PER_NOTE {
            return Err(CoreError::TooManyAttachments);
        }
        let attachment = Attachment {
            id: Uuid::now_v7().to_string(),
            note_id: note_id.to_string(),
            mime: mime_for(&name, mime),
            name,
            size: bytes.len() as u64,
            created_at: now,
        };
        tx.execute(
            "INSERT INTO attachments (id, note_id, name, mime, size, data, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                attachment.id,
                attachment.note_id,
                attachment.name,
                attachment.mime,
                attachment.size as i64,
                bytes,
                now
            ],
        )?;
        tx.execute(
            "UPDATE notes SET updated_at = ?1 WHERE id = ?2",
            params![now, note_id],
        )?;
        tx.commit()?;
        Ok(attachment)
    }

    /// An attachment and its bytes. Files of trashed notes are not served.
    pub fn attachment_bytes(&self, id: &str) -> Result<(Attachment, Vec<u8>)> {
        self.conn()
            .query_row(
                &format!(
                    "SELECT {ATTACHMENT_COLUMNS}, a.data FROM attachments a
                     JOIN notes n ON n.id = a.note_id
                     WHERE a.id = ?1 AND n.deleted_at IS NULL"
                ),
                [id],
                |r| Ok((attachment_from_row(r)?, r.get::<_, Vec<u8>>(6)?)),
            )
            .optional()?
            .ok_or(CoreError::NotFound)
    }

    pub fn remove_attachment(&mut self, id: &str) -> Result<()> {
        let now = now_ms();
        let tx = self.conn_mut().transaction()?;
        let note_id: Option<String> = tx
            .query_row(
                "SELECT a.note_id FROM attachments a JOIN notes n ON n.id = a.note_id
                 WHERE a.id = ?1 AND n.deleted_at IS NULL",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        let Some(note_id) = note_id else {
            return Err(CoreError::NotFound);
        };
        tx.execute("DELETE FROM attachments WHERE id = ?1", [id])?;
        tx.execute(
            "UPDATE notes SET updated_at = ?1 WHERE id = ?2",
            params![now, note_id],
        )?;
        tx.commit()?;
        Ok(())
    }
}

fn attachment_from_row(r: &Row<'_>) -> rusqlite::Result<Attachment> {
    Ok(Attachment {
        id: r.get(0)?,
        note_id: r.get(1)?,
        name: r.get(2)?,
        mime: r.get(3)?,
        size: r.get::<_, i64>(4)?.max(0) as u64,
        created_at: r.get(5)?,
    })
}

/// Normalises line endings and enforces the length limit. With `truncate`
/// (imports) an over-long description is cut instead of refused.
pub(crate) fn clean_details(details: &str, truncate: bool) -> Result<String> {
    let text = details.replace("\r\n", "\n");
    if text.trim().is_empty() {
        return Ok(String::new());
    }
    if text.chars().count() > MAX_DETAILS_CHARS {
        if truncate {
            return Ok(text.chars().take(MAX_DETAILS_CHARS).collect());
        }
        return Err(CoreError::DetailsTooLong);
    }
    Ok(text)
}

/// A display name safe to store and show: no path, no control characters,
/// at most [`MAX_NAME_CHARS`], keeping the extension when it has to cut.
pub(crate) fn clean_file_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = base.chars().filter(|c| !c.is_control()).collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        return "file".into();
    }
    if cleaned.chars().count() <= MAX_NAME_CHARS {
        return cleaned.to_string();
    }
    let ext = extension(cleaned)
        .filter(|e| e.chars().count() <= 10)
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    let keep = MAX_NAME_CHARS - ext.chars().count();
    let stem: String = cleaned.chars().take(keep).collect();
    format!("{}{ext}", stem.trim_end())
}

fn extension(name: &str) -> Option<&str> {
    let (stem, ext) = name.rsplit_once('.')?;
    (!stem.is_empty() && !ext.is_empty()).then_some(ext)
}

/// The type a file is shown and saved as.
pub(crate) fn mime_for(name: &str, hint: &str) -> String {
    let by_ext = extension(name)
        .map(|e| e.to_ascii_lowercase())
        .and_then(|e| {
            Some(match e.as_str() {
                "png" => "image/png",
                "jpg" | "jpeg" => "image/jpeg",
                "gif" => "image/gif",
                "webp" => "image/webp",
                "bmp" => "image/bmp",
                "svg" => "image/svg+xml",
                "heic" => "image/heic",
                "pdf" => "application/pdf",
                "txt" | "log" => "text/plain",
                "md" | "markdown" => "text/markdown",
                "csv" => "text/csv",
                "json" => "application/json",
                "zip" => "application/zip",
                "doc" => "application/msword",
                "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                "xls" => "application/vnd.ms-excel",
                "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                "ppt" => "application/vnd.ms-powerpoint",
                "pptx" => {
                    "application/vnd.openxmlformats-officedocument.presentationml.presentation"
                }
                _ => return None,
            })
        });
    if let Some(m) = by_ext {
        return m.to_string();
    }
    let hint = hint.trim();
    let plausible = hint.len() <= 100
        && hint.contains('/')
        && hint
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "/+-.".contains(c));
    if plausible {
        hint.to_ascii_lowercase()
    } else {
        "application/octet-stream".into()
    }
}
