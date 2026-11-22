//! Portable export and import: Markdown to read anywhere, JSON for tools.

use std::path::Path;

use rusqlite::params;
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::db::{clean_category, clean_content, note_from_row, now_ms, Db, NOTE_COLUMNS};
use crate::details::clean_details;
use crate::error::{CoreError, Result};
use crate::markdown;
use crate::steps::clean_step_text;
use crate::sync::record::{record as sync_record, record_step_parts};
use crate::sync::{Field, StepPart};
use posthaste_model::{ImportReport, Status};

pub const EXPORT_FORMAT: &str = "posthaste-export";
pub const EXPORT_VERSION: u32 = 2;

#[derive(Debug, Serialize, Deserialize)]
struct ExportFile {
    format: String,
    version: u32,
    exported_at: String,
    notes: Vec<ExportNote>,
}

/// One note in the export file. Timestamps are RFC 3339 UTC strings so the
/// file is readable without Posthaste.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ExportNote {
    #[serde(default)]
    pub(crate) id: Option<String>,
    pub(crate) content: String,
    #[serde(default)]
    pub(crate) status: Option<String>,
    #[serde(default)]
    pub(crate) category: Option<String>,
    #[serde(default)]
    pub(crate) source: Option<String>,
    #[serde(default)]
    pub(crate) created_at: Option<String>,
    #[serde(default)]
    pub(crate) updated_at: Option<String>,
    #[serde(default)]
    pub(crate) completed_at: Option<String>,
    #[serde(default)]
    pub(crate) archived_at: Option<String>,
    /// When the note was parked in Later; absent when it is not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) later_at: Option<String>,
    /// The note's description. Attached files are not exported.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) details: String,
    /// The note's steps, tombstones included: JSON keeps what Markdown
    /// cannot show.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) steps: Vec<ExportStep>,
}

/// One step in an export file.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ExportStep {
    pub(crate) id: String,
    pub(crate) text: String,
    pub(crate) done: bool,
    pub(crate) rank: String,
    /// `true` for a removed step kept as a tombstone.
    pub(crate) deleted: bool,
}

fn to_rfc3339(ms: i64) -> String {
    OffsetDateTime::from_unix_timestamp_nanos(ms as i128 * 1_000_000)
        .ok()
        .and_then(|t| t.format(&Rfc3339).ok())
        .unwrap_or_default()
}

fn from_rfc3339(s: &str) -> Option<i64> {
    OffsetDateTime::parse(s, &Rfc3339)
        .ok()
        .map(|t| (t.unix_timestamp_nanos() / 1_000_000) as i64)
}

/// Which kind of file an export is written as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// A readable task list; dates are shown in the given time zone.
    Markdown {
        utc_offset_minutes: i32,
    },
    Json,
}

impl Db {
    fn export_notes(&self) -> Result<Vec<ExportNote>> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {NOTE_COLUMNS}, details
             FROM notes WHERE deleted_at IS NULL ORDER BY created_at, seq"
        ))?;
        let rows = stmt
            .query_map([], |r| Ok((note_from_row(r)?, r.get::<_, String>(14)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(stmt);
        let mut notes = Vec::with_capacity(rows.len());
        for (n, details) in rows {
            notes.push(ExportNote {
                id: Some(n.id.clone()),
                content: n.content,
                status: Some(n.status.as_str().to_string()),
                category: n.category,
                source: n.source,
                created_at: Some(to_rfc3339(n.created_at)),
                updated_at: Some(to_rfc3339(n.updated_at)),
                completed_at: n.completed_at.map(to_rfc3339),
                archived_at: n.archived_at.map(to_rfc3339),
                later_at: n.later_at.map(to_rfc3339),
                details,
                steps: self.export_steps(&n.id)?,
            });
        }
        Ok(notes)
    }

    /// A note's steps for the export file, tombstones included, in display
    /// order.
    fn export_steps(&self, note_id: &str) -> Result<Vec<ExportStep>> {
        let mut stmt = self.conn().prepare_cached(
            "SELECT id, text, done, rank, deleted_at IS NOT NULL FROM note_steps
             WHERE note_id = ?1
             ORDER BY rank, id",
        )?;
        let rows = stmt.query_map([note_id], |r| {
            Ok(ExportStep {
                id: r.get(0)?,
                text: r.get(1)?,
                done: r.get(2)?,
                rank: r.get(3)?,
                deleted: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn export_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(&ExportFile {
            format: EXPORT_FORMAT.into(),
            version: EXPORT_VERSION,
            exported_at: to_rfc3339(now_ms()),
            notes: self.export_notes()?,
        })?)
    }

    pub fn export_markdown(&self, utc_offset_minutes: i32) -> Result<String> {
        Ok(markdown::write(
            &self.export_notes()?,
            &to_rfc3339(now_ms()),
            utc_offset_minutes,
        ))
    }

    /// Writes the export to `dest` via a temporary file, so a failed write
    /// never leaves a truncated export behind.
    pub fn export_to(&self, dest: &Path, format: ExportFormat) -> Result<usize> {
        let text = match format {
            ExportFormat::Markdown { utc_offset_minutes } => {
                self.export_markdown(utc_offset_minutes)?
            }
            ExportFormat::Json => self.export_json()?,
        };
        let count = self.info()?.note_count as usize;
        let mut tmp = dest.as_os_str().to_owned();
        tmp.push(".part");
        let tmp = std::path::PathBuf::from(tmp);
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, dest)?;
        Ok(count)
    }

    pub fn import_from(&mut self, src: &Path) -> Result<ImportReport> {
        self.import_text(&std::fs::read_to_string(src)?)
    }

    /// Imports either kind of export, telling them apart by their content.
    pub fn import_text(&mut self, text: &str) -> Result<ImportReport> {
        let text = text.trim_start_matches('\u{feff}');
        if text.trim_start().starts_with('{') {
            self.import_json(text)
        } else {
            self.import_markdown(text)
        }
    }

    pub fn import_markdown(&mut self, text: &str) -> Result<ImportReport> {
        let notes = markdown::read(text)?;
        self.import_notes(notes)
    }

    pub fn import_json(&mut self, json: &str) -> Result<ImportReport> {
        let file: ExportFile =
            serde_json::from_str(json).map_err(|e| CoreError::InvalidImport(e.to_string()))?;
        if file.format != EXPORT_FORMAT {
            return Err(CoreError::InvalidImport(format!(
                "unexpected format \"{}\"",
                file.format
            )));
        }
        if file.version > EXPORT_VERSION {
            return Err(CoreError::InvalidImport(format!(
                "export version {} is newer than this version of Posthaste supports",
                file.version
            )));
        }
        self.import_notes(file.notes)
    }

    /// Merges notes into the database. Notes whose id already exists are
    /// left untouched; nothing is ever overwritten. All or nothing.
    fn import_notes(&mut self, notes: Vec<ExportNote>) -> Result<ImportReport> {
        let now = now_ms();
        let mut report = ImportReport {
            imported: 0,
            skipped_existing: 0,
            skipped_invalid: 0,
            skipped_steps: 0,
        };
        let tx = self.conn_mut().transaction()?;
        {
            let mut insert = tx.prepare(
                "INSERT OR IGNORE INTO notes
                   (id, content, status, category, source, created_at, updated_at,
                    completed_at, archived_at, details, later_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            )?;
            for n in notes {
                let content = match clean_content(&n.content) {
                    Ok(c) => c,
                    Err(_) => {
                        report.skipped_invalid += 1;
                        continue;
                    }
                };
                let status = match n.status.as_deref() {
                    None => Status::Inbox,
                    Some(s) => match Status::parse(s) {
                        Some(s) => s,
                        None => {
                            report.skipped_invalid += 1;
                            continue;
                        }
                    },
                };
                let ts = |v: &Option<String>| v.as_deref().and_then(from_rfc3339);
                let created = ts(&n.created_at).unwrap_or(now);
                let completed =
                    ts(&n.completed_at).or((status == Status::Completed).then_some(created));
                let archived =
                    ts(&n.archived_at).or((status == Status::Archived).then_some(created));
                let id =
                    n.id.filter(|id| !id.trim().is_empty())
                        .unwrap_or_else(|| Uuid::now_v7().to_string());
                let cleaned_category = clean_category(n.category.as_deref());
                let cleaned_details = clean_details(&n.details, true).unwrap_or_default();
                let later_val = (status == Status::Inbox).then(|| ts(&n.later_at)).flatten();
                let updated = ts(&n.updated_at).unwrap_or(created);
                let inserted = insert.execute(params![
                    id,
                    content,
                    status.as_str(),
                    cleaned_category,
                    n.source,
                    created,
                    updated,
                    completed,
                    archived,
                    cleaned_details,
                    // Only an open note can be parked.
                    later_val,
                ])?;
                if inserted == 1 {
                    let mut fields = vec![Field::Created, Field::Content];
                    if cleaned_category.is_some() {
                        fields.push(Field::Category);
                    }
                    if status != Status::Inbox {
                        fields.push(Field::Status);
                    }
                    if !cleaned_details.is_empty() {
                        fields.push(Field::Details);
                    }
                    if later_val.is_some() {
                        fields.push(Field::Later);
                    }
                    sync_record(&tx, &id, &fields, updated)?;
                    report.skipped_steps += import_steps(&tx, &id, &n.steps, updated)?;
                    report.imported += 1;
                } else {
                    report.skipped_existing += 1;
                }
            }
        }
        tx.commit()?;
        Ok(report)
    }
}

/// Writes a note's exported steps into the database, with their ids, ranks
/// and tombstones. Steps that fail validation are counted and reported,
/// never dropped silently. Steps follow the note: they are only written
/// when the note itself was new.
fn import_steps(
    tx: &rusqlite::Transaction<'_>,
    note_id: &str,
    steps: &[ExportStep],
    now: i64,
) -> Result<usize> {
    let mut skipped = 0;
    for step in steps {
        let Ok(text) = clean_step_text(&step.text) else {
            skipped += 1;
            continue;
        };
        // A hand-typed step has no id: one is made for it here.
        let id = if step.id.trim().is_empty() {
            Uuid::now_v7().to_string()
        } else {
            step.id.clone()
        };
        tx.execute(
            "INSERT OR IGNORE INTO note_steps
               (id, note_id, text, done, rank, created_at, updated_at, deleted_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?7)",
            params![
                id,
                note_id,
                text,
                step.done as i64,
                step.rank,
                now,
                (step.deleted).then_some(now),
            ],
        )?;
        let mut parts = vec![StepPart::Text, StepPart::Done, StepPart::Rank];
        if step.deleted {
            parts.push(StepPart::Deleted);
        }
        record_step_parts(tx, note_id, &step.id, &parts, now)?;
    }
    Ok(skipped)
}
