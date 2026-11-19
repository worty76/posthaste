use std::collections::HashMap;

use crate::{Db, NewNote, Status};

fn db_with_device() -> Db {
    let mut db = Db::open_in_memory().unwrap();
    db.set_sync_device_id("dev-a").unwrap();
    db
}

fn note(content: &str) -> NewNote {
    NewNote {
        content: content.into(),
        category: None,
        source: None,
    }
}

fn fields_for(db: &Db, note_id: &str) -> HashMap<String, i64> {
    let mut stmt = db
        .conn()
        .prepare("SELECT field, seq FROM sync_fields WHERE note_id = ?1")
        .unwrap();
    stmt.query_map([note_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
        .unwrap()
        .collect::<std::result::Result<HashMap<_, _>, _>>()
        .unwrap()
}

fn total_sync_fields(db: &Db) -> i64 {
    db.conn()
        .query_row("SELECT COUNT(*) FROM sync_fields", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn without_device_id_writes_record_nothing() {
    let mut db = Db::open_in_memory().unwrap();
    let n = db.create_note_at(note("plain"), 1_000).unwrap();
    db.update_note_at(&n.id, "updated", Some("work"), 2_000)
        .unwrap();
    db.set_status_at(&n.id, Status::Completed, 3_000).unwrap();
    db.set_later_at(&n.id, true, 4_000).unwrap();
    db.set_details_at(&n.id, "desc", 5_000).unwrap();
    db.trash_note_at(&n.id, 6_000).unwrap();
    db.restore_trashed_at(&n.id, 7_000).unwrap();
    db.trash_note_at(&n.id, 8_000).unwrap();
    db.purge_note(&n.id).unwrap();
    assert_eq!(total_sync_fields(&db), 0);
}

#[test]
fn create_and_capture_record_initial_fields() {
    let mut db = db_with_device();
    let n1 = db
        .create_note_at(
            NewNote {
                content: "one".into(),
                category: Some("work".into()),
                source: None,
            },
            1_000,
        )
        .unwrap();
    let f1 = fields_for(&db, &n1.id);
    assert!(f1.contains_key("created"));
    assert!(f1.contains_key("content"));
    assert!(f1.contains_key("category"));

    let n2 = db.capture_note_at(note("two"), 2_000).unwrap();
    let f2 = fields_for(&db, &n2.id);
    assert!(f2["content"] > f1["content"]);
    assert!(f2.contains_key("created"));
    assert!(!f2.contains_key("category"));
}

#[test]
fn update_status_later_details_trash_restore_record_advancing_seqs() {
    let mut db = db_with_device();
    let n = db.create_note_at(note("hello"), 1_000).unwrap();
    let s0 = fields_for(&db, &n.id)["content"];

    db.update_note_at(&n.id, "hello 2", Some("tag"), 2_000)
        .unwrap();
    let f = fields_for(&db, &n.id);
    assert!(f["content"] > s0);
    assert!(f["category"] > s0);
    let s1 = f["content"];

    db.set_status_at(&n.id, Status::Completed, 3_000).unwrap();
    let f = fields_for(&db, &n.id);
    assert!(f["status"] > s1);
    let s2 = f["status"];

    db.set_later_at(&n.id, true, 4_000).unwrap();
    let f = fields_for(&db, &n.id);
    assert!(f["later"] > s2);
    assert!(f["status"] > s2);
    let s3 = f["later"];

    db.set_details_at(&n.id, "more info", 5_000).unwrap();
    let f = fields_for(&db, &n.id);
    assert!(f["details"] > s3);
    let s4 = f["details"];

    db.trash_note_at(&n.id, 6_000).unwrap();
    let f = fields_for(&db, &n.id);
    assert!(f["deleted"] > s4);
    let s5 = f["deleted"];

    db.restore_trashed_at(&n.id, 7_000).unwrap();
    let f = fields_for(&db, &n.id);
    assert!(f["deleted"] > s5);
}

#[test]
fn purge_methods_record_purged_tombstone() {
    let mut db = db_with_device();

    // purge_note
    let n1 = db.create_note_at(note("one"), 1_000).unwrap();
    db.trash_note_at(&n1.id, 2_000).unwrap();
    let s_before = fields_for(&db, &n1.id)["deleted"];
    db.purge_note(&n1.id).unwrap();
    assert!(fields_for(&db, &n1.id)["purged"] > s_before);

    // purge_trashed
    let n2 = db.create_note_at(note("two"), 1_000).unwrap();
    db.trash_note_at(&n2.id, 2_000).unwrap();
    assert_eq!(db.purge_trashed(3_000).unwrap(), 1);
    assert!(fields_for(&db, &n2.id).contains_key("purged"));

    // empty_trash_at
    let n3 = db.create_note_at(note("three"), 1_000).unwrap();
    db.trash_note_at(&n3.id, 10_000).unwrap();
    assert_eq!(db.empty_trash_at(10_000, 11_000).unwrap(), 1);
    assert!(fields_for(&db, &n3.id).contains_key("purged"));
}

#[test]
fn import_notes_records_imported_fields() {
    let mut db = db_with_device();
    let json = r#"{"format":"posthaste-export","version":1,"exported_at":"2026-01-01T00:00:00Z",
        "notes":[{"id":"imp-1","content":"imported note","category":"work","details":"body"}]}"#;
    let report = db.import_json(json).unwrap();
    assert_eq!(report.imported, 1);
    let f = fields_for(&db, "imp-1");
    assert!(f.contains_key("created"));
    assert!(f.contains_key("content"));
    assert!(f.contains_key("category"));
    assert!(f.contains_key("details"));
}

#[test]
fn a_write_that_fails_records_nothing() {
    let mut db = db_with_device();
    let n = db.create_note_at(note("keep me"), 1_000).unwrap();
    let before = fields_for(&db, &n.id);

    assert!(db.update_note_at(&n.id, "   ", None, 2_000).is_err());
    assert_eq!(fields_for(&db, &n.id), before);
    assert!(db.set_status_at("missing-id", Status::Completed, 2_000).is_err());
    assert!(fields_for(&db, "missing-id").is_empty());
}

#[test]
fn clear_all_wipes_sync_fields_and_peers() {
    let mut db = db_with_device();
    db.create_note_at(note("gone"), 1_000).unwrap();
    db.upsert_peer("dev-b", "Other", &[], 1_000).unwrap();
    assert!(total_sync_fields(&db) > 0);
    assert_eq!(db.list_peers().unwrap().len(), 1);

    db.clear_all().unwrap();
    assert_eq!(total_sync_fields(&db), 0);
    assert!(db.list_peers().unwrap().is_empty());
}
