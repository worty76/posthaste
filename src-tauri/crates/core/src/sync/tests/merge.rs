use crate::sync::CONFLICT_SOURCE;
use crate::{Db, NewNote, NoteQuery, Status};

fn setup_pair() -> (Db, Db) {
    let mut a = Db::open_in_memory().unwrap();
    let mut b = Db::open_in_memory().unwrap();
    a.set_sync_device_id("dev-a").unwrap();
    b.set_sync_device_id("dev-b").unwrap();
    a.upsert_peer("dev-b", "B", &[], 1_000).unwrap();
    b.upsert_peer("dev-a", "A", &[], 1_000).unwrap();
    (a, b)
}

fn note(content: &str) -> NewNote {
    NewNote {
        content: content.into(),
        category: None,
        source: None,
    }
}

fn sync_one_way(src: &mut Db, src_id: &str, dst: &mut Db, dst_id: &str, now_ms: i64) {
    let since = src
        .get_peer(dst_id)
        .unwrap()
        .map(|p| p.sent_up_to)
        .unwrap_or(0);
    let (changes, up_to) = src.changes_since(since, 1000).unwrap();
    if !changes.is_empty() {
        dst.apply_changes(src_id, &changes, now_ms).unwrap();
        src.update_peer_sent_up_to(dst_id, up_to, now_ms).unwrap();
    }
}

fn sync_both_ways(a: &mut Db, b: &mut Db, now_ms: i64) {
    sync_one_way(a, "dev-a", b, "dev-b", now_ms);
    sync_one_way(b, "dev-b", a, "dev-a", now_ms);
}

#[test]
fn a_new_note_reaches_the_other_device() {
    let (mut a, mut b) = setup_pair();
    let n = a
        .create_note_at(
            NewNote {
                content: "buy milk".into(),
                category: Some("home".into()),
                source: None,
            },
            5_000,
        )
        .unwrap();
    a.set_details_at(&n.id, "2% organic", 5_100).unwrap();

    sync_one_way(&mut a, "dev-a", &mut b, "dev-b", 6_000);

    let got = b.note_detail(&n.id).unwrap();
    assert_eq!(got.note.content, "buy milk");
    assert_eq!(got.note.category.as_deref(), Some("home"));
    assert_eq!(got.note.created_at, 5_000);
    assert_eq!(got.details, "2% organic");
}

#[test]
fn applying_the_same_batch_twice_changes_nothing() {
    let (mut a, mut b) = setup_pair();
    let n = a.create_note_at(note("idempotent"), 1_000).unwrap();
    let (changes, _) = a.changes_since(0, 100).unwrap();

    let first = b.apply_changes("dev-a", &changes, 2_000).unwrap();
    assert!(first.applied >= 2);
    assert_eq!(first.conflicts, 0);

    let second = b.apply_changes("dev-a", &changes, 2_100).unwrap();
    assert_eq!(second.applied, 0);
    assert_eq!(second.skipped, changes.len());
    assert_eq!(second.conflicts, 0);
    assert_eq!(b.get_note(&n.id).unwrap().content, "idempotent");
    assert_eq!(b.list_notes(&NoteQuery::default()).unwrap().len(), 1);
}

#[test]
fn the_later_status_wins_and_status_fields_move_together() {
    let (mut a, mut b) = setup_pair();
    let n = a.create_note_at(note("task"), 1_000).unwrap();
    sync_both_ways(&mut a, &mut b, 1_500);

    // A completes at t=2000; B archives later at t=3000.
    a.set_status_at(&n.id, Status::Completed, 2_000).unwrap();
    b.set_status_at(&n.id, Status::Archived, 3_000).unwrap();

    sync_both_ways(&mut a, &mut b, 4_000);

    let na = a.get_note(&n.id).unwrap();
    let nb = b.get_note(&n.id).unwrap();
    assert_eq!(na.status, Status::Archived);
    assert_eq!(nb.status, Status::Archived);
    assert_eq!(na.archived_at, Some(3_000));
    assert_eq!(nb.archived_at, Some(3_000));
    assert_eq!(na.completed_at, None);
    assert_eq!(nb.completed_at, None);
}

#[test]
fn both_sides_edit_the_text_and_both_texts_survive() {
    let (mut a, mut b) = setup_pair();
    let n = a.create_note_at(note("original"), 1_000).unwrap();
    sync_both_ways(&mut a, &mut b, 1_500);

    // Both edit while apart. B's edit is later, so B wins the note; A's text
    // must survive as a `sync-conflict` note on both devices.
    a.update_note_at(&n.id, "edit from A", None, 2_000).unwrap();
    b.update_note_at(&n.id, "edit from B", None, 3_000).unwrap();

    // Sync A -> B, then B -> A, then A -> B so the conflict note propagates.
    sync_both_ways(&mut a, &mut b, 4_000);
    sync_both_ways(&mut a, &mut b, 4_500);

    assert_eq!(a.get_note(&n.id).unwrap().content, "edit from B");
    assert_eq!(b.get_note(&n.id).unwrap().content, "edit from B");

    let notes_a = a.list_notes(&NoteQuery::default()).unwrap();
    let notes_b = b.list_notes(&NoteQuery::default()).unwrap();

    let conflicts_a: Vec<&str> = notes_a
        .iter()
        .filter(|x| x.source.as_deref() == Some(CONFLICT_SOURCE))
        .map(|x| x.content.as_str())
        .collect();
    let conflicts_b: Vec<&str> = notes_b
        .iter()
        .filter(|x| x.source.as_deref() == Some(CONFLICT_SOURCE))
        .map(|x| x.content.as_str())
        .collect();

    assert!(
        conflicts_a.contains(&"edit from A"),
        "expected A to preserve losing text, got {conflicts_a:?}"
    );
    assert!(
        conflicts_b.contains(&"edit from A"),
        "expected B to preserve losing text, got {conflicts_b:?}"
    );
}

#[test]
fn an_erased_note_stays_erased_and_a_newer_edit_is_kept() {
    let (mut a, mut b) = setup_pair();
    let n = a.create_note_at(note("shared"), 1_000).unwrap();
    sync_both_ways(&mut a, &mut b, 1_500);

    // A trashes and purges the note while B is offline and edits it.
    a.trash_note_at(&n.id, 2_000).unwrap();
    a.purge_note(&n.id).unwrap();

    b.update_note_at(&n.id, "B edited while offline", None, 5_000)
        .unwrap();

    sync_both_ways(&mut a, &mut b, 6_000);
    sync_both_ways(&mut a, &mut b, 6_500);

    // The erased note stays erased on both sides.
    assert!(a.get_note(&n.id).is_err());
    assert!(b.get_note(&n.id).is_err());

    // B's newer text survives as a sync-conflict note.
    let notes_b = b.list_notes(&NoteQuery::default()).unwrap();
    assert!(notes_b
        .iter()
        .any(|x| x.content == "B edited while offline"
            && x.source.as_deref() == Some(CONFLICT_SOURCE)));
    let notes_a = a.list_notes(&NoteQuery::default()).unwrap();
    assert!(notes_a
        .iter()
        .any(|x| x.content == "B edited while offline"));
}

#[test]
fn trash_and_restore_follow_the_later_change() {
    let (mut a, mut b) = setup_pair();
    let n = a.create_note_at(note("fragile"), 1_000).unwrap();
    sync_both_ways(&mut a, &mut b, 1_500);

    a.trash_note_at(&n.id, 2_000).unwrap();
    sync_both_ways(&mut a, &mut b, 2_500);
    assert!(b.get_note(&n.id).is_err());
    assert_eq!(b.list_trashed_at(2_500).unwrap().len(), 1);

    b.restore_trashed_at(&n.id, 3_000).unwrap();
    sync_both_ways(&mut a, &mut b, 3_500);
    assert_eq!(a.get_note(&n.id).unwrap().content, "fragile");
    assert!(a.list_trashed_at(3_500).unwrap().is_empty());
}

#[test]
fn three_devices_converge() {
    let mut a = Db::open_in_memory().unwrap();
    let mut b = Db::open_in_memory().unwrap();
    let mut c = Db::open_in_memory().unwrap();
    a.set_sync_device_id("dev-a").unwrap();
    b.set_sync_device_id("dev-b").unwrap();
    c.set_sync_device_id("dev-c").unwrap();
    for (db, peers) in [
        (&mut a, ["dev-b", "dev-c"]),
        (&mut b, ["dev-a", "dev-c"]),
        (&mut c, ["dev-a", "dev-b"]),
    ] {
        for p in peers {
            db.upsert_peer(p, p, &[], 1_000).unwrap();
        }
    }

    let n1 = a.create_note_at(note("from A"), 1_000).unwrap();
    let n2 = b.create_note_at(note("from B"), 1_100).unwrap();
    let n3 = c.create_note_at(note("from C"), 1_200).unwrap();

    // Chain A -> B -> C -> A, then one more pass so everyone has everything.
    sync_one_way(&mut a, "dev-a", &mut b, "dev-b", 2_000);
    b.set_status_at(&n1.id, Status::Completed, 2_100).unwrap();
    sync_one_way(&mut b, "dev-b", &mut c, "dev-c", 2_200);
    c.set_details_at(&n2.id, "noted on C", 2_300).unwrap();
    sync_one_way(&mut c, "dev-c", &mut a, "dev-a", 2_400);
    sync_one_way(&mut a, "dev-a", &mut b, "dev-b", 2_500);
    sync_one_way(&mut b, "dev-b", &mut c, "dev-c", 2_600);

    let snapshot = |db: &Db| -> Vec<(String, String, Status, String)> {
        let mut out: Vec<_> = db
            .list_notes(&NoteQuery::default())
            .unwrap()
            .into_iter()
            .map(|n| {
                let d = db.note_detail(&n.id).unwrap().details;
                (n.id, n.content, n.status, d)
            })
            .collect();
        out.sort_by(|x, y| x.0.cmp(&y.0));
        out
    };

    let sa = snapshot(&a);
    let sb = snapshot(&b);
    let sc = snapshot(&c);
    assert_eq!(sa.len(), 3);
    assert_eq!(sa, sb);
    assert_eq!(sb, sc);
    assert_eq!(a.get_note(&n1.id).unwrap().status, Status::Completed);
    assert_eq!(a.note_detail(&n2.id).unwrap().details, "noted on C");
    assert_eq!(b.get_note(&n3.id).unwrap().content, "from C");
}
