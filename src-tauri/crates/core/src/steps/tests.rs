//! Tests for the fractional ranks that order steps, and for steps stored
//! in the database.

use super::rank::rank_between;
use crate::migrations::{self, MIGRATIONS};
use crate::*;
use rusqlite::Connection;

/// A small deterministic generator, so the tests need no dependency.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        // xorshift64*, adequate for choosing insert positions.
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
}

#[test]
fn rank_between_none_and_none_is_not_empty() {
    let rank = rank_between(None, None);
    assert!(!rank.is_empty());
    assert!(rank.chars().all(|c| c.is_ascii_alphanumeric()));
}

#[test]
fn rank_between_sorts_strictly_between_its_neighbours() {
    let mut random = Random(0x5EED_2026_1121);
    let mut ranks: Vec<String> = Vec::new();
    for _ in 0..2_000 {
        // Insert at a random position of the growing list.
        let at = random.below(ranks.len() + 1);
        let before = if at == 0 {
            None
        } else {
            Some(ranks[at - 1].as_str())
        };
        let after = ranks.get(at).map(|r| r.as_str());
        let rank = rank_between(before, after);
        if let Some(before) = before {
            assert!(rank.as_bytes() > before.as_bytes(), "{rank} <= {before}");
        }
        if let Some(after) = after {
            assert!(rank.as_bytes() < after.as_bytes(), "{rank} >= {after}");
        }
        ranks.insert(at, rank);
    }
    // The whole list stays sorted, so the same data always reads the same.
    let sorted = ranks.clone();
    let mut sorted = sorted;
    sorted.sort();
    assert_eq!(ranks, sorted);
}

#[test]
fn rank_before_the_first_and_after_the_last_stay_ordered() {
    let mut ranks: Vec<String> = Vec::new();
    for _ in 0..500 {
        let rank = rank_between(None, ranks.first().map(|r| r.as_str()));
        if let Some(first) = ranks.first() {
            assert!(rank.as_bytes() < first.as_bytes());
        }
        ranks.insert(0, rank);
    }
    for _ in 0..500 {
        let rank = rank_between(ranks.last().map(|r| r.as_str()), None);
        if let Some(last) = ranks.last() {
            assert!(rank.as_bytes() > last.as_bytes());
        }
        ranks.push(rank);
    }
    let mut sorted = ranks.clone();
    sorted.sort();
    assert_eq!(ranks, sorted);
}

#[test]
fn ranks_never_end_in_the_lowest_digit() {
    // A rank ending in the lowest digit would leave no room to insert
    // directly before it but after the rank it follows.
    let mut random = Random(99);
    let mut ranks: Vec<String> = Vec::new();
    for _ in 0..500 {
        let at = random.below(ranks.len() + 1);
        let before = if at == 0 {
            None
        } else {
            Some(ranks[at - 1].as_str())
        };
        let rank = rank_between(before, ranks.get(at).map(|r| r.as_str()));
        assert!(!rank.ends_with('0'), "{rank}");
        ranks.insert(at, rank);
    }
}

// ---- steps in the database ---------------------------------------------------

fn step(text: &str) -> NewStep {
    NewStep {
        text: text.into(),
        done: false,
    }
}

fn a_note(db: &mut Db) -> String {
    db.create_note(NewNote {
        content: "a note".into(),
        ..Default::default()
    })
    .unwrap()
    .id
}

fn texts(steps: &[Step]) -> Vec<&str> {
    steps.iter().map(|s| s.text.as_str()).collect()
}

/// A note with `how_many` steps, the first `done_count` of them ticked.
fn note_with_steps(db: &mut Db, how_many: usize, done_count: usize) -> String {
    let note = a_note(db);
    let lines: Vec<NewStep> = (0..how_many)
        .map(|i| NewStep {
            text: format!("step {i}"),
            done: i < done_count,
        })
        .collect();
    db.add_steps(&note, None, &lines).unwrap();
    note
}

#[test]
fn a_blank_step_is_refused() {
    let mut db = Db::open_in_memory().unwrap();
    let id = note_with_steps(&mut db, 1, 0);
    for blank in ["", "   ", "\t"] {
        assert!(
            matches!(
                db.add_steps(&id, None, &[step(blank)]),
                Err(CoreError::StepInvalid)
            ),
            "{blank:?}"
        );
    }
    assert_eq!(db.list_steps(&id).unwrap().len(), 1);
}

#[test]
fn step_text_rejects_multiline_content() {
    let mut db = Db::open_in_memory().unwrap();
    let id = note_with_steps(&mut db, 1, 0);
    for multiline in ["one\ntwo", "one\r\ntwo", "one\rtwo"] {
        assert!(matches!(
            db.add_steps(&id, None, &[step(multiline)]),
            Err(CoreError::StepInvalid)
        ));
    }
    assert_eq!(db.list_steps(&id).unwrap().len(), 1);
}

#[test]
fn step_text_rejects_more_than_500_characters() {
    let mut db = Db::open_in_memory().unwrap();
    let id = note_with_steps(&mut db, 1, 0);
    // Character counts, not bytes: 500 Vietnamese letters with diacritics pass.
    let long = format!("{}{}", "uộàảềễ".repeat(83), "uộ");
    assert_eq!(long.chars().count(), 500);
    let accepted = db.add_steps(&id, None, &[step(&long)]).unwrap();
    let too_long = format!("{}{}", "uộàảềễ".repeat(83), "uộà");
    assert_eq!(too_long.chars().count(), 501);
    assert!(matches!(
        db.set_step_text(&accepted[0].id, &too_long),
        Err(CoreError::StepInvalid)
    ));
    assert!(matches!(
        db.add_steps(&id, None, &[step(&too_long)]),
        Err(CoreError::StepInvalid)
    ));
    // The steps that were accepted are still there, whole.
    assert_eq!(db.list_steps(&id).unwrap().len(), 2);
    assert_eq!(db.list_steps(&id).unwrap()[0].text.chars().count(), 500);
}

#[test]
fn adding_steps_keeps_the_order_given_and_ids_stay_stable_after_reordering() {
    let mut db = Db::open_in_memory().unwrap();
    let id = note_with_steps(&mut db, 2, 0);
    let steps = db.list_steps(&id).unwrap();
    assert_eq!(texts(&steps), ["step 0", "step 1"]);
    let ids: Vec<&str> = steps.iter().map(|s| s.id.as_str()).collect();

    // Move the second step to the front; ids never change.
    let moved = db.move_step(&steps[1].id, None).unwrap();
    assert_eq!(moved.id, steps[1].id);
    let reordered = db.list_steps(&id).unwrap();
    assert_eq!(texts(&reordered), ["step 1", "step 0"]);
    assert_eq!(reordered[0].id, steps[1].id);
    assert_eq!(reordered[1].id, steps[0].id);
    assert_ne!(ids, Vec::<&str>::new());

    // Insert after the first step.
    let added = db
        .add_steps(&id, Some(&reordered[0].id), &[step("between")])
        .unwrap();
    assert_eq!(
        texts(&db.list_steps(&id).unwrap()),
        ["step 1", "between", "step 0"]
    );
    assert_eq!(added.len(), 1);
}

#[test]
fn a_local_add_past_100_steps_is_refused() {
    let mut db = Db::open_in_memory().unwrap();
    let id = note_with_steps(&mut db, MAX_STEPS_PER_NOTE, 0);
    let err = db
        .add_steps(&id, None, &[step("one too many")])
        .unwrap_err();
    assert_eq!(err.code(), "step_limit");
    assert_eq!(db.list_steps(&id).unwrap().len(), MAX_STEPS_PER_NOTE);
}

#[test]
fn an_oversized_local_paste_is_rejected_whole_without_data_loss() {
    let mut db = Db::open_in_memory().unwrap();
    let id = note_with_steps(&mut db, 99, 0);
    let before: Vec<String> = db
        .list_steps(&id)
        .unwrap()
        .iter()
        .map(|s| s.id.clone())
        .collect();
    let paste: Vec<NewStep> = (0..5).map(|i| step(&format!("pasted {i}"))).collect();
    assert_eq!(
        db.add_steps(&id, None, &paste).unwrap_err().code(),
        "step_limit"
    );
    let after: Vec<String> = db
        .list_steps(&id)
        .unwrap()
        .iter()
        .map(|s| s.id.clone())
        .collect();
    assert_eq!(after.len(), 99);
    assert_eq!(after, before);
}

#[test]
fn equal_ranks_order_by_id() {
    let mut db = Db::open_in_memory().unwrap();
    let id = a_note(&mut db);
    // Two rows with the same rank, written straight past the API.
    db.conn_mut()
        .execute(
            "INSERT INTO note_steps (id, note_id, text, done, rank, created_at, updated_at)
             VALUES ('b', ?1, 'second', 0, 'M', 1, 1),
                    ('a', ?1, 'first', 0, 'M', 1, 1)",
            [&id],
        )
        .unwrap();
    assert_eq!(
        texts(&db.list_steps(&id).unwrap()),
        ["first", "second"],
        "equal ranks tie-break by id"
    );
}

#[test]
fn deleted_steps_are_excluded_from_counts_and_lists() {
    let mut db = Db::open_in_memory().unwrap();
    let id = note_with_steps(&mut db, 3, 1);
    let steps = db.list_steps(&id).unwrap();
    db.remove_step(&steps[2].id).unwrap();
    let note = db.get_note(&id).unwrap();
    assert_eq!(note.step_total, 2);
    assert_eq!(note.step_done, 1);
    assert_eq!(texts(&db.list_steps(&id).unwrap()), ["step 0", "step 1"]);
}

#[test]
fn restoring_a_removed_step_brings_it_back_unchanged() {
    let mut db = Db::open_in_memory().unwrap();
    let id = note_with_steps(&mut db, 3, 1);
    let steps = db.list_steps(&id).unwrap();
    let (text, rank, done) = (steps[2].text.clone(), steps[2].rank.clone(), steps[2].done);
    db.remove_step(&steps[2].id).unwrap();
    let restored = db.restore_step(&steps[2].id).unwrap();
    assert_eq!((restored.text.clone(), restored.done), (text, done));
    assert_eq!(restored.rank, rank);
    assert_eq!(
        texts(&db.list_steps(&id).unwrap()),
        ["step 0", "step 1", "step 2"]
    );
}

#[test]
fn ticking_every_step_leaves_the_note_open_and_every_step_done() {
    let mut db = Db::open_in_memory().unwrap();
    let id = note_with_steps(&mut db, 2, 0);
    for step in db.list_steps(&id).unwrap() {
        db.set_step_done(&step.id, true).unwrap();
    }
    let steps = db.list_steps(&id).unwrap();
    assert!(steps.iter().all(|s| s.done));
    // The note itself never completes itself.
    assert_eq!(db.get_note(&id).unwrap().status, Status::Inbox);
    let note = db.get_note(&id).unwrap();
    assert_eq!(note.step_total, 2);
    assert_eq!(note.step_done, 2);
}

#[test]
fn list_payload_carries_step_counts_excluding_deleted() {
    let mut db = Db::open_in_memory().unwrap();
    let id = note_with_steps(&mut db, 3, 2);
    let steps = db.list_steps(&id).unwrap();
    db.remove_step(&steps[0].id).unwrap();

    let listed = db.list_notes(&NoteQuery::default()).unwrap();
    let note = listed.iter().find(|n| n.id == id).unwrap();
    assert_eq!(note.step_total, 2);
    assert_eq!(note.step_done, 1);

    // The Today window's rows come from glance; they carry the same counts.
    let glance = db.glance(0, i64::MAX).unwrap();
    let today = glance.today.iter().find(|n| n.id == id).unwrap();
    assert_eq!(today.step_total, 2);
    assert_eq!(today.step_done, 1);
}

#[test]
fn a_done_or_later_note_allows_step_edits() {
    let mut db = Db::open_in_memory().unwrap();
    let done_note = note_with_steps(&mut db, 1, 0);
    db.set_status(&done_note, Status::Completed).unwrap();
    let step = db.list_steps(&done_note).unwrap().remove(0);
    let edited = db.set_step_text(&step.id, "still editable").unwrap();
    assert_eq!(edited.text, "still editable");

    let later_note = note_with_steps(&mut db, 1, 0);
    db.set_later(&later_note, true).unwrap();
    let step = db.list_steps(&later_note).unwrap().remove(0);
    db.set_step_done(&step.id, true).unwrap();
    assert!(db.list_steps(&later_note).unwrap()[0].done);
}

#[test]
fn a_trashed_note_rejects_step_edits() {
    let mut db = Db::open_in_memory().unwrap();
    let id = note_with_steps(&mut db, 2, 0);
    db.trash_note(&id).unwrap();
    let existing = db.list_steps(&id).unwrap().remove(0);
    for err in [
        db.set_step_text(&existing.id, "nope").unwrap_err().code(),
        db.set_step_done(&existing.id, true).unwrap_err().code(),
        db.move_step(&existing.id, None).unwrap_err().code(),
        db.remove_step(&existing.id).unwrap_err().code(),
    ] {
        assert_eq!(err, "note_trashed");
    }
    assert!(matches!(
        db.add_steps(&id, None, &[step("nope")]),
        Err(CoreError::NoteTrashed)
    ));
    // Nothing changed.
    assert_eq!(texts(&db.list_steps(&id).unwrap()), ["step 0", "step 1"]);
}

#[test]
fn restoring_a_note_from_trash_keeps_ids_text_order_and_done_state() {
    let mut db = Db::open_in_memory().unwrap();
    let id = note_with_steps(&mut db, 3, 2);
    let before = db.list_steps(&id).unwrap();
    db.trash_note(&id).unwrap();
    db.restore_trashed(&id).unwrap();
    let after = db.list_steps(&id).unwrap();
    assert_eq!(before, after);
}

#[test]
fn purging_a_note_deletes_its_steps() {
    let mut db = Db::open_in_memory().unwrap();
    let id = note_with_steps(&mut db, 2, 0);
    db.trash_note(&id).unwrap();
    db.purge_note(&id).unwrap();
    let left: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM note_steps WHERE note_id = ?1",
            [&id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(left, 0, "purge removes the steps with the note");
}

#[test]
fn migration_adds_note_steps_to_an_existing_database() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("posthaste.db");
    {
        // A database as the previous version of the app would have left it.
        let mut conn = Connection::open(&path).unwrap();
        migrations::migrate_with(&mut conn, Some(&path), &MIGRATIONS[..MIGRATIONS.len() - 1])
            .unwrap();
        conn.execute(
            "INSERT INTO notes (id, content, created_at, updated_at)
             VALUES ('legacy-steps', 'written before steps existed', 1, 1)",
            [],
        )
        .unwrap();
    }

    let db = Db::open(&path).unwrap();
    assert_eq!(
        db.info().unwrap().schema_version,
        MIGRATIONS.last().unwrap().version
    );
    let note = db.get_note("legacy-steps").unwrap();
    assert_eq!(note.step_total, 0);
    assert_eq!(note.step_done, 0);
    assert_eq!(db.list_steps("legacy-steps").unwrap(), [] as [Step; 0]);
}
