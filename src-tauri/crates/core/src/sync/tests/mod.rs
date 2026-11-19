use rusqlite::Connection;

use crate::migrations::{self, MIGRATIONS};
use crate::sync::Hlc;
use crate::{Db, NewNote, Status};

fn note(content: &str) -> NewNote {
    NewNote {
        content: content.into(),
        category: None,
        source: None,
    }
}

#[test]
fn hlc_strings_sort_like_clocks() {
    let a = Hlc::new(1_700_000_000_000, 0, "dev-a");
    let b = Hlc::new(1_700_000_000_000, 0, "dev-b");
    let c = Hlc::new(1_700_000_000_000, 1, "dev-a");
    let d = Hlc::new(1_700_000_000_001, 0, "dev-a");

    let list = [&a, &b, &c, &d];
    for pair in list.windows(2) {
        assert!(pair[0] < pair[1]);
        assert!(pair[0].format() < pair[1].format());
    }
    assert_eq!(Hlc::parse(&c.format()), Some(c));
}

#[test]
fn hlc_never_goes_backwards_when_the_wall_clock_does() {
    let first = Hlc::tick(None, 2_000_000, "dev-a");
    assert_eq!(first, Hlc::new(2_000_000, 0, "dev-a"));

    let jumped_back = Hlc::tick(Some(&first), 1_000_000, "dev-a");
    assert_eq!(jumped_back, Hlc::new(2_000_000, 1, "dev-a"));
    assert!(jumped_back.format() > first.format());

    let same_ms = Hlc::tick(Some(&jumped_back), 2_000_000, "dev-a");
    assert_eq!(same_ms, Hlc::new(2_000_000, 2, "dev-a"));

    let advanced = Hlc::tick(Some(&same_ms), 2_000_001, "dev-a");
    assert_eq!(advanced, Hlc::new(2_000_001, 0, "dev-a"));
}

#[test]
fn observing_a_clock_a_day_ahead_moves_forward_and_stays_ordered() {
    let now = 1_700_000_000_000;
    let one_day = 86_400_000;
    let local = Hlc::tick(None, now, "dev-a");
    let far_future = Hlc::new(now + one_day, 5, "dev-b");

    let observed = Hlc::observe(Some(&local), &far_future, now, "dev-a");
    assert_eq!(observed.wall_ms, now + one_day);
    assert_eq!(observed.counter, 6);
    assert!(observed.format() > far_future.format());
    assert!(observed.format() > local.format());

    // Subsequent local tick with real wall clock still never moves backwards.
    let next = Hlc::tick(Some(&observed), now + 100, "dev-a");
    assert_eq!(next.wall_ms, now + one_day);
    assert_eq!(next.counter, 7);
    assert!(next.format() > observed.format());
}

#[test]
fn migration_7_creates_the_sync_tables_and_keeps_notes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("posthaste.db");
    {
        let mut conn = Connection::open(&path).unwrap();
        migrations::migrate_with(&mut conn, Some(&path), &MIGRATIONS[..6]).unwrap();
        conn.execute(
            "INSERT INTO notes (id, content, created_at, updated_at)
             VALUES ('legacy-6', 'written before sync existed', 1000, 1000)",
            [],
        )
        .unwrap();
    }

    let db = Db::open(&path).unwrap();
    assert_eq!(db.info().unwrap().schema_version, 7);
    assert_eq!(
        db.get_note("legacy-6").unwrap().content,
        "written before sync existed"
    );

    for table in ["sync_fields", "sync_peers", "sync_state"] {
        let count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [table],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "missing table {table}");
    }
}

#[test]
fn setting_the_device_id_backfills_existing_notes() {
    let mut db = Db::open_in_memory().unwrap();
    assert_eq!(db.sync_device_id().unwrap(), None);

    let n1 = db
        .create_note_at(
            NewNote {
                content: "first".into(),
                category: Some("work".into()),
                source: None,
            },
            1_000,
        )
        .unwrap();
    let n2 = db.create_note_at(note("second"), 2_000).unwrap();
    db.set_details_at(&n2.id, "extra details", 2_500).unwrap();
    db.set_status_at(&n2.id, Status::Completed, 2_600).unwrap();

    let n3 = db.create_note_at(note("trashed"), 3_000).unwrap();
    db.set_later_at(&n3.id, true, 3_100).unwrap();
    db.trash_note_at(&n3.id, 3_200).unwrap();

    db.set_sync_device_id("dev-1").unwrap();
    assert_eq!(db.sync_device_id().unwrap().as_deref(), Some("dev-1"));

    // Calling again is a no-op and does not overwrite device_id.
    db.set_sync_device_id("dev-2").unwrap();
    assert_eq!(db.sync_device_id().unwrap().as_deref(), Some("dev-1"));

    let mut stmt = db
        .conn()
        .prepare("SELECT note_id, field, hlc, seq FROM sync_fields ORDER BY seq ASC")
        .unwrap();
    let rows: Vec<(String, String, String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<std::result::Result<_, _>>()
        .unwrap();

    // Distinct strictly ascending seqs.
    for pair in rows.windows(2) {
        assert!(pair[0].3 < pair[1].3);
    }

    let fields_for = |id: &str| -> Vec<String> {
        rows.iter()
            .filter(|(nid, _, _, _)| nid == id)
            .map(|(_, f, _, _)| f.clone())
            .collect()
    };

    assert_eq!(fields_for(&n1.id), vec!["created", "content", "category"]);
    assert_eq!(
        fields_for(&n2.id),
        vec!["created", "content", "details", "status"]
    );
    assert_eq!(
        fields_for(&n3.id),
        vec!["created", "content", "later", "deleted"]
    );
}
