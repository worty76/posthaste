//! Projects: named groups of related notes, backed by `notes.category` and
//! the local `projects` table for archived state and empty projects.

use rusqlite::{params, OptionalExtension, Transaction};

use crate::db::{clean_category, now_ms, Db};
use crate::error::{CoreError, Result};
use crate::sync::record::record as sync_record;
use crate::sync::Field;
use posthaste_model::{Note, Project};

fn clean_project_name(name: &str) -> Result<String> {
    clean_category(Some(name)).ok_or(CoreError::ProjectInvalid)
}

fn project_exists(tx: &Transaction<'_>, name: &str) -> Result<bool> {
    let exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM projects WHERE name = ?1)
             OR EXISTS(SELECT 1 FROM notes WHERE category = ?1 AND deleted_at IS NULL)",
        [name],
        |r| r.get(0),
    )?;
    Ok(exists)
}

impl Db {
    /// Lists projects: the union of `projects` rows and distinct live
    /// `category` values, ordered by newest note (`last_at` DESC) then name.
    pub fn projects(&self, include_archived: bool) -> Result<Vec<Project>> {
        let mut stmt = self.conn().prepare_cached(
            "WITH names AS (
                 SELECT name FROM projects
                 UNION
                 SELECT DISTINCT category AS name FROM notes
                 WHERE deleted_at IS NULL AND category IS NOT NULL
             )
             SELECT
                 n.name,
                 COALESCE(p.archived_at IS NOT NULL, 0) AS archived,
                 COUNT(notes.id) FILTER (WHERE notes.status = 'inbox') AS open_count,
                 COUNT(notes.id) AS total_count,
                 COALESCE(MAX(notes.created_at), 0) AS last_at
             FROM names n
             LEFT JOIN projects p ON p.name = n.name
             LEFT JOIN notes ON notes.category = n.name AND notes.deleted_at IS NULL
             WHERE ?1 OR p.archived_at IS NULL
             GROUP BY n.name
             ORDER BY last_at DESC, n.name ASC",
        )?;
        let rows = stmt.query_map([include_archived], |r| {
            Ok(Project {
                name: r.get(0)?,
                archived: r.get::<_, i64>(1)? != 0,
                open: r.get::<_, i64>(2)?.max(0) as u32,
                total: r.get::<_, i64>(3)?.max(0) as u32,
                last_at: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn create_project(&mut self, name: &str) -> Result<Project> {
        self.create_project_at(name, now_ms())
    }

    /// Creates an empty project row. Refuses a blank or existing name.
    pub fn create_project_at(&mut self, name: &str, now: i64) -> Result<Project> {
        let name = clean_project_name(name)?;
        let tx = self.conn_mut().transaction()?;
        if project_exists(&tx, &name)? {
            return Err(CoreError::ProjectExists);
        }
        tx.execute(
            "INSERT INTO projects (name, archived_at, created_at) VALUES (?1, NULL, ?2)",
            params![name, now],
        )?;
        tx.commit()?;
        Ok(Project {
            name,
            archived: false,
            open: 0,
            total: 0,
            last_at: 0,
        })
    }

    pub fn rename_project(&mut self, from: &str, to: &str, merge: bool) -> Result<u32> {
        self.rename_project_at(from, to, merge, now_ms())
    }

    /// Renames a project across every matching note and its `projects` row in
    /// one transaction, recording `Field::Category` per moved note for sync.
    pub fn rename_project_at(
        &mut self,
        from: &str,
        to: &str,
        merge: bool,
        now: i64,
    ) -> Result<u32> {
        let from = clean_project_name(from)?;
        let to = clean_project_name(to)?;
        if from == to {
            return Ok(0);
        }
        let tx = self.conn_mut().transaction()?;
        if !project_exists(&tx, &from)? {
            return Err(CoreError::NotFound);
        }
        if project_exists(&tx, &to)? && !merge {
            return Err(CoreError::ProjectExists);
        }

        let ids: Vec<String> = {
            let mut stmt =
                tx.prepare("SELECT id FROM notes WHERE category = ?1 AND deleted_at IS NULL")?;
            let rows = stmt.query_map([&from], |r| r.get(0))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        for id in &ids {
            tx.execute(
                "UPDATE notes SET category = ?1, updated_at = ?2 WHERE id = ?3",
                params![to, now, id],
            )?;
            sync_record(&tx, id, &[Field::Category], now)?;
        }

        let old_row: Option<(Option<i64>, i64)> = tx
            .query_row(
                "SELECT archived_at, created_at FROM projects WHERE name = ?1",
                [&from],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        tx.execute("DELETE FROM projects WHERE name = ?1", [&from])?;
        let (archived_at, created_at) = old_row.unwrap_or((None, now));
        tx.execute(
            "INSERT OR IGNORE INTO projects (name, archived_at, created_at)
             VALUES (?1, ?2, ?3)",
            params![to, archived_at, created_at],
        )?;
        tx.commit()?;
        Ok(ids.len() as u32)
    }

    pub fn set_project_archived(&mut self, name: &str, archived: bool) -> Result<()> {
        self.set_project_archived_at(name, archived, now_ms())
    }

    /// Marks a project archived or active, creating its `projects` row if
    /// only notes carried the name.
    pub fn set_project_archived_at(&mut self, name: &str, archived: bool, now: i64) -> Result<()> {
        let name = clean_project_name(name)?;
        let archived_at = archived.then_some(now);
        self.conn().execute(
            "INSERT INTO projects (name, archived_at, created_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(name) DO UPDATE SET archived_at = excluded.archived_at",
            params![name, archived_at, now],
        )?;
        Ok(())
    }

    pub fn set_note_project(&mut self, note_id: &str, project: Option<&str>) -> Result<Note> {
        self.set_note_project_at(note_id, project, now_ms())
    }

    /// Sets or clears a note's project, recording `Field::Category` for sync
    /// and unarchiving the target project row if it was archived.
    pub fn set_note_project_at(
        &mut self,
        note_id: &str,
        project: Option<&str>,
        now: i64,
    ) -> Result<Note> {
        let project = clean_category(project);
        let tx = self.conn_mut().transaction()?;
        let changed = tx.execute(
            "UPDATE notes SET category = ?1, updated_at = ?2
             WHERE id = ?3 AND deleted_at IS NULL",
            params![project, now, note_id],
        )?;
        if changed == 0 {
            return Err(CoreError::NotFound);
        }
        sync_record(&tx, note_id, &[Field::Category], now)?;
        if let Some(ref name) = project {
            tx.execute(
                "INSERT INTO projects (name, archived_at, created_at) VALUES (?1, NULL, ?2)
                 ON CONFLICT(name) DO UPDATE SET archived_at = NULL",
                params![name, now],
            )?;
        }
        tx.commit()?;
        self.get_note(note_id)
    }

    pub fn clear_project(&mut self, name: &str) -> Result<u32> {
        self.clear_project_at(name, now_ms())
    }

    /// Removes the project from every note that has it (recording
    /// `Field::Category` for sync) and deletes its `projects` row.
    pub fn clear_project_at(&mut self, name: &str, now: i64) -> Result<u32> {
        let name = clean_project_name(name)?;
        let tx = self.conn_mut().transaction()?;
        let ids: Vec<String> = {
            let mut stmt =
                tx.prepare("SELECT id FROM notes WHERE category = ?1 AND deleted_at IS NULL")?;
            let rows = stmt.query_map([&name], |r| r.get(0))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        for id in &ids {
            tx.execute(
                "UPDATE notes SET category = NULL, updated_at = ?1 WHERE id = ?2",
                params![now, id],
            )?;
            sync_record(&tx, id, &[Field::Category], now)?;
        }
        tx.execute("DELETE FROM projects WHERE name = ?1", [&name])?;
        tx.commit()?;
        Ok(ids.len() as u32)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use rusqlite::Connection;

    use crate::migrations::{self, MIGRATIONS};
    use crate::sync::{decode_frame, encode_frame, Message, Session};
    use crate::{Db, NewNote, Project, Status};

    fn db() -> Db {
        Db::open_in_memory().unwrap()
    }

    fn note_in(content: &str, project: Option<&str>) -> NewNote {
        NewNote {
            content: content.into(),
            category: project.map(Into::into),
            source: None,
        }
    }

    #[test]
    fn migration_creates_a_project_for_every_category_in_use() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("posthaste.db");
        {
            let mut conn = Connection::open(&path).unwrap();
            migrations::migrate_with(&mut conn, Some(&path), &MIGRATIONS[..8]).unwrap();
            conn.execute(
                "INSERT INTO notes (id, content, status, category, created_at, updated_at)
                 VALUES ('n1', 'file taxes', 'inbox', 'taxes', 1000, 1000),
                        ('n2', 'receipts', 'completed', 'taxes', 2000, 2000),
                        ('n3', 'pack tent', 'inbox', 'camping', 1500, 1500)",
                [],
            )
            .unwrap();
        }
        let db = Db::open(&path).unwrap();
        let rows: Vec<(String, i64)> = {
            let mut stmt = db
                .conn()
                .prepare("SELECT name, created_at FROM projects ORDER BY name")
                .unwrap();
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap()
        };
        assert_eq!(rows, vec![("camping".into(), 1500), ("taxes".into(), 1000)]);
    }

    #[test]
    fn projects_list_counts_open_and_total_and_orders_by_latest_note() {
        let mut db = db();
        let n1 = db
            .create_note_at(note_in("first tax", Some("taxes")), 1_000)
            .unwrap();
        db.set_status_at(&n1.id, Status::Completed, 1_100).unwrap();
        db.create_note_at(note_in("second tax", Some("taxes")), 3_000)
            .unwrap();
        db.create_note_at(note_in("buy stove", Some("camping")), 2_000)
            .unwrap();

        let list = db.projects(false).unwrap();
        assert_eq!(
            list,
            vec![
                Project {
                    name: "taxes".into(),
                    archived: false,
                    open: 1,
                    total: 2,
                    last_at: 3_000,
                },
                Project {
                    name: "camping".into(),
                    archived: false,
                    open: 1,
                    total: 1,
                    last_at: 2_000,
                },
            ]
        );
    }

    #[test]
    fn an_empty_created_project_is_listed_and_a_trashed_only_project_is_not() {
        let mut db = db();
        let created = db.create_project_at("empty", 1_000).unwrap();
        assert_eq!(
            created,
            Project {
                name: "empty".into(),
                archived: false,
                open: 0,
                total: 0,
                last_at: 0,
            }
        );

        // A note whose project has no `projects` row and is then trashed
        // disappears from the project list.
        let trashed = db
            .create_note_at(note_in("old idea", Some("ephemeral")), 2_000)
            .unwrap();
        db.trash_note_at(&trashed.id, 2_500).unwrap();

        let names: Vec<String> = db
            .projects(true)
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(names, vec!["empty".to_string()]);
    }

    #[test]
    fn restoring_a_trashed_note_brings_its_project_back() {
        let mut db = db();
        let n = db
            .create_note_at(note_in("tent pegs", Some("camping")), 1_000)
            .unwrap();
        db.trash_note_at(&n.id, 2_000).unwrap();
        assert!(db.projects(true).unwrap().is_empty());

        db.restore_trashed_at(&n.id, 3_000).unwrap();
        let list = db.projects(false).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "camping");
        assert_eq!(list[0].open, 1);
    }

    #[test]
    fn archived_projects_are_left_out_unless_asked() {
        let mut db = db();
        db.create_note_at(note_in("done trip", Some("trip")), 1_000)
            .unwrap();
        db.create_note_at(note_in("active work", Some("work")), 2_000)
            .unwrap();
        db.set_project_archived_at("trip", true, 3_000).unwrap();

        let active: Vec<String> = db
            .projects(false)
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(active, vec!["work".to_string()]);

        let all = db.projects(true).unwrap();
        assert_eq!(all.len(), 2);
        let trip = all.iter().find(|p| p.name == "trip").unwrap();
        assert!(trip.archived);
    }

    #[test]
    fn renaming_moves_every_note_and_records_category_for_sync() {
        let mut db = db();
        db.set_sync_device_id("dev-a").unwrap();
        let a = db
            .create_note_at(note_in("one", Some("old")), 1_000)
            .unwrap();
        let b = db
            .create_note_at(note_in("two", Some("old")), 2_000)
            .unwrap();
        let before_a: String = db
            .conn()
            .query_row(
                "SELECT hlc FROM sync_fields WHERE note_id = ?1 AND field = 'category'",
                [&a.id],
                |r| r.get(0),
            )
            .unwrap();

        let moved = db.rename_project_at("old", "new", false, 5_000).unwrap();
        assert_eq!(moved, 2);
        assert_eq!(db.get_note(&a.id).unwrap().category.as_deref(), Some("new"));
        assert_eq!(db.get_note(&b.id).unwrap().category.as_deref(), Some("new"));

        let after_a: String = db
            .conn()
            .query_row(
                "SELECT hlc FROM sync_fields WHERE note_id = ?1 AND field = 'category'",
                [&a.id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(after_a > before_a);

        let names: Vec<String> = db
            .projects(true)
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(names, vec!["new".to_string()]);
    }

    #[test]
    fn renaming_onto_an_existing_name_needs_merge() {
        let mut db = db();
        let a = db
            .create_note_at(note_in("one", Some("alpha")), 1_000)
            .unwrap();
        let b = db
            .create_note_at(note_in("two", Some("beta")), 2_000)
            .unwrap();

        let err = db
            .rename_project_at("alpha", "beta", false, 3_000)
            .unwrap_err();
        assert_eq!(err.code(), "project_exists");
        assert_eq!(
            db.get_note(&a.id).unwrap().category.as_deref(),
            Some("alpha")
        );

        let moved = db.rename_project_at("alpha", "beta", true, 4_000).unwrap();
        assert_eq!(moved, 1);
        assert_eq!(
            db.get_note(&a.id).unwrap().category.as_deref(),
            Some("beta")
        );
        assert_eq!(
            db.get_note(&b.id).unwrap().category.as_deref(),
            Some("beta")
        );
        let list = db.projects(true).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "beta");
        assert_eq!(list[0].total, 2);
    }

    #[test]
    fn setting_a_notes_project_to_an_archived_name_unarchives_it() {
        let mut db = db();
        db.create_project_at("taxes", 1_000).unwrap();
        db.set_project_archived_at("taxes", true, 2_000).unwrap();
        let n = db.create_note_at(note_in("w2 form", None), 3_000).unwrap();

        let updated = db.set_note_project_at(&n.id, Some("taxes"), 4_000).unwrap();
        assert_eq!(updated.category.as_deref(), Some("taxes"));

        let list = db.projects(false).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "taxes");
        assert!(!list[0].archived);
    }

    #[test]
    fn clearing_a_project_leaves_its_notes_with_none_and_drops_the_row() {
        let mut db = db();
        db.create_project_at("taxes", 1_000).unwrap();
        let n = db
            .create_note_at(note_in("file return", Some("taxes")), 2_000)
            .unwrap();

        let cleared = db.clear_project_at("taxes", 3_000).unwrap();
        assert_eq!(cleared, 1);
        assert_eq!(db.get_note(&n.id).unwrap().category, None);
        assert!(db.projects(true).unwrap().is_empty());
    }

    #[test]
    fn a_project_name_is_cleaned_like_a_category() {
        let mut db = db();
        let p = db.create_project_at("  #home-repair  ", 1_000).unwrap();
        assert_eq!(p.name, "home-repair");

        let dup = db.create_project_at("home-repair", 2_000).unwrap_err();
        assert_eq!(dup.code(), "project_exists");

        for blank in ["", "   ", "#", " #  "] {
            let err = db.create_project_at(blank, 3_000).unwrap_err();
            assert_eq!(err.code(), "project_invalid");
        }
    }

    fn sync_pair(a: &mut Db, b: &mut Db, now_ms: i64) {
        let mut sa = Session::new("dev-a", "dev-b").with_name("A");
        let mut sb = Session::new("dev-b", "dev-a").with_name("B");
        let mut q_a_to_b: VecDeque<Message> = sa.start(a).unwrap().into();
        let mut q_b_to_a: VecDeque<Message> = sb.start(b).unwrap().into();
        while !q_a_to_b.is_empty() || !q_b_to_a.is_empty() {
            if let Some(msg) = q_a_to_b.pop_front() {
                let decoded = decode_frame(&encode_frame(&msg).unwrap()).unwrap();
                q_b_to_a.extend(sb.on_message(b, decoded, now_ms).unwrap());
            }
            if let Some(msg) = q_b_to_a.pop_front() {
                let decoded = decode_frame(&encode_frame(&msg).unwrap()).unwrap();
                q_a_to_b.extend(sa.on_message(a, decoded, now_ms).unwrap());
            }
        }
    }

    #[test]
    fn a_renamed_project_reaches_a_paired_device_as_category_changes() {
        let mut a = db();
        let mut b = db();
        a.set_sync_device_id("dev-a").unwrap();
        b.set_sync_device_id("dev-b").unwrap();
        a.upsert_peer("dev-b", "B", &[], 1_000).unwrap();
        b.upsert_peer("dev-a", "A", &[], 1_000).unwrap();

        let n = a
            .create_note_at(note_in("pay accountant", Some("tax-2025")), 2_000)
            .unwrap();
        sync_pair(&mut a, &mut b, 3_000);
        assert_eq!(
            b.get_note(&n.id).unwrap().category.as_deref(),
            Some("tax-2025")
        );

        // Device A renames the project while device B edits the note's text.
        a.rename_project_at("tax-2025", "taxes", false, 4_000)
            .unwrap();
        b.set_details_at(&n.id, "bring receipt", 4_500).unwrap();
        sync_pair(&mut a, &mut b, 5_000);

        // Both devices end with the renamed project and the new details,
        // never a blank project.
        assert_eq!(
            a.get_note(&n.id).unwrap().category.as_deref(),
            Some("taxes")
        );
        assert_eq!(
            b.get_note(&n.id).unwrap().category.as_deref(),
            Some("taxes")
        );
        assert_eq!(a.note_detail(&n.id).unwrap().details, "bring receipt");
    }
}
