use rusqlite::Connection;

use crate::migrations::{self, Migration, MIGRATIONS};
use crate::*;

fn db() -> Db {
    Db::open_in_memory().unwrap()
}

fn note(content: &str) -> NewNote {
    NewNote {
        content: content.into(),
        ..Default::default()
    }
}

fn contents(notes: &[Note]) -> Vec<&str> {
    notes.iter().map(|n| n.content.as_str()).collect()
}

fn search(db: &Db, text: &str) -> Vec<Note> {
    db.list_notes(&NoteQuery {
        text: text.into(),
        ..Default::default()
    })
    .unwrap()
}

// ---- creation ------------------------------------------------------------

#[test]
fn creates_a_note_in_the_inbox() {
    let mut db = db();
    let n = db
        .create_note_at(
            NewNote {
                content: "  Investigate PostgreSQL indexing strategy.  ".into(),
                category: Some(" #work ".into()),
                source: Some("Code".into()),
            },
            1_000,
        )
        .unwrap();
    assert_eq!(n.content, "Investigate PostgreSQL indexing strategy.");
    assert_eq!(n.status, Status::Inbox);
    assert_eq!(n.category.as_deref(), Some("work"));
    assert_eq!(n.source.as_deref(), Some("Code"));
    assert_eq!((n.created_at, n.updated_at), (1_000, 1_000));
    assert_eq!(db.get_note(&n.id).unwrap(), n);
}

#[test]
fn rejects_empty_and_whitespace_only_notes() {
    let mut db = db();
    for blank in ["", "   ", "\n\t \n"] {
        assert!(matches!(
            db.create_note(note(blank)),
            Err(CoreError::EmptyContent)
        ));
    }
    assert_eq!(db.list_notes(&NoteQuery::default()).unwrap().len(), 0);
}

#[test]
fn keeps_multiline_content_and_normalises_line_endings() {
    let mut db = db();
    let n = db.create_note(note("first\r\nsecond\n\nfourth")).unwrap();
    assert_eq!(n.content, "first\nsecond\n\nfourth");
}

#[test]
fn blank_category_is_stored_as_none() {
    let mut db = db();
    let n = db
        .create_note(NewNote {
            content: "x".into(),
            category: Some("  ".into()),
            source: None,
        })
        .unwrap();
    assert_eq!(n.category, None);
}

// ---- editing -------------------------------------------------------------

#[test]
fn edits_content_and_category() {
    let mut db = db();
    let n = db.create_note_at(note("draft wording"), 1_000).unwrap();
    let edited = db
        .update_note_at(&n.id, "final wording", Some("ideas"), 2_000)
        .unwrap();
    assert_eq!(edited.content, "final wording");
    assert_eq!(edited.category.as_deref(), Some("ideas"));
    assert_eq!(edited.created_at, 1_000);
    assert_eq!(edited.updated_at, 2_000);
}

#[test]
fn editing_to_empty_is_rejected_and_keeps_the_note() {
    let mut db = db();
    let n = db.create_note(note("keep me")).unwrap();
    assert!(matches!(
        db.update_note(&n.id, "   ", None),
        Err(CoreError::EmptyContent)
    ));
    assert_eq!(db.get_note(&n.id).unwrap().content, "keep me");
}

#[test]
fn editing_a_missing_note_reports_not_found() {
    let mut db = db();
    assert!(matches!(
        db.update_note("nope", "x", None),
        Err(CoreError::NotFound)
    ));
}

// ---- completion and archiving ---------------------------------------------

#[test]
fn completing_sets_and_reopening_clears_the_timestamp() {
    let mut db = db();
    let n = db.create_note_at(note("task"), 1_000).unwrap();

    let done = db.set_status_at(&n.id, Status::Completed, 2_000).unwrap();
    assert_eq!(done.status, Status::Completed);
    assert_eq!(done.completed_at, Some(2_000));

    let reopened = db.set_status_at(&n.id, Status::Inbox, 3_000).unwrap();
    assert_eq!(reopened.status, Status::Inbox);
    assert_eq!(reopened.completed_at, None);
    assert_eq!(reopened.archived_at, None);
}

#[test]
fn archiving_and_restoring() {
    let mut db = db();
    let n = db.create_note_at(note("reference"), 1_000).unwrap();

    let archived = db.set_status_at(&n.id, Status::Archived, 2_000).unwrap();
    assert_eq!(archived.status, Status::Archived);
    assert_eq!(archived.archived_at, Some(2_000));

    let inbox = NoteQuery {
        statuses: vec![Status::Inbox],
        ..Default::default()
    };
    assert!(db.list_notes(&inbox).unwrap().is_empty());

    db.set_status_at(&n.id, Status::Inbox, 3_000).unwrap();
    assert_eq!(db.list_notes(&inbox).unwrap().len(), 1);
}

// ---- deletion --------------------------------------------------------------

#[test]
fn trashed_notes_disappear_but_can_be_restored() {
    let mut db = db();
    let n = db.create_note(note("oops")).unwrap();
    db.trash_note_at(&n.id, 5_000).unwrap();

    assert!(matches!(db.get_note(&n.id), Err(CoreError::NotFound)));
    assert!(db.list_notes(&NoteQuery::default()).unwrap().is_empty());
    assert!(search(&db, "oops").is_empty());

    let back = db.restore_trashed_at(&n.id, 6_000).unwrap();
    assert_eq!(back, n);
    assert_eq!(search(&db, "oops").len(), 1);
}

#[test]
fn purge_removes_only_notes_trashed_before_the_cutoff() {
    let mut db = db();
    let old = db.create_note(note("old")).unwrap();
    let recent = db.create_note(note("recent")).unwrap();
    let live = db.create_note(note("live")).unwrap();
    db.trash_note_at(&old.id, 1_000).unwrap();
    db.trash_note_at(&recent.id, 9_000).unwrap();

    assert_eq!(db.purge_trashed(5_000).unwrap(), 1);
    assert!(matches!(
        db.restore_trashed_at(&old.id, 10_000),
        Err(CoreError::NotFound)
    ));
    assert!(db.restore_trashed_at(&recent.id, 10_000).is_ok());
    assert!(db.get_note(&live.id).is_ok());
    // The purged note is gone from the search index too.
    assert!(search(&db, "old").is_empty());
}

// ---- listing ---------------------------------------------------------------

#[test]
fn lists_newest_first_with_stable_order_for_equal_timestamps() {
    let mut db = db();
    db.create_note_at(note("one"), 1_000).unwrap();
    db.create_note_at(note("two"), 2_000).unwrap();
    db.create_note_at(note("three-a"), 3_000).unwrap();
    db.create_note_at(note("three-b"), 3_000).unwrap();
    let all = db.list_notes(&NoteQuery::default()).unwrap();
    assert_eq!(contents(&all), ["three-b", "three-a", "two", "one"]);
}

#[test]
fn filters_by_status_and_date_range_and_paginates() {
    let mut db = db();
    let a = db.create_note_at(note("a"), 1_000).unwrap();
    db.create_note_at(note("b"), 2_000).unwrap();
    db.create_note_at(note("c"), 3_000).unwrap();
    db.set_status(&a.id, Status::Completed).unwrap();

    let completed = db
        .list_notes(&NoteQuery {
            statuses: vec![Status::Completed, Status::Archived],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(contents(&completed), ["a"]);

    // from is inclusive, to is exclusive
    let range = db
        .list_notes(&NoteQuery {
            created_from: Some(2_000),
            created_to: Some(3_000),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(contents(&range), ["b"]);

    let page = db
        .list_notes(&NoteQuery {
            limit: Some(1),
            offset: Some(1),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(contents(&page), ["b"]);
}

#[test]
fn counts_follow_status_and_the_callers_day() {
    let mut db = db();
    let a = db.create_note_at(note("yesterday"), 500).unwrap();
    let b = db.create_note_at(note("today 1"), 1_500).unwrap();
    db.create_note_at(note("today 2"), 1_600).unwrap();
    let d = db.create_note_at(note("deleted"), 1_700).unwrap();
    db.set_status(&a.id, Status::Archived).unwrap();
    db.set_status(&b.id, Status::Completed).unwrap();
    db.trash_note(&d.id).unwrap();

    assert_eq!(
        db.counts(1_000, 2_000).unwrap(),
        Counts {
            inbox: 1,
            today: 2,
            later: 0,
            archive: 2,
            trash: 1
        }
    );
    assert_eq!(
        Db::open_in_memory().unwrap().counts(0, 1).unwrap(),
        Counts {
            inbox: 0,
            today: 0,
            later: 0,
            archive: 0,
            trash: 0
        }
    );
}

#[test]
fn categories_are_listed_most_used_first() {
    let mut db = db();
    for (content, cat) in [("a", "work"), ("b", "ideas"), ("c", "work")] {
        db.create_note(NewNote {
            content: content.into(),
            category: Some(cat.into()),
            source: None,
        })
        .unwrap();
    }
    db.create_note(note("uncategorised")).unwrap();
    assert_eq!(db.categories().unwrap(), ["work", "ideas"]);
}

// ---- category filter ---------------------------------------------------------

fn by_category(db: &Db, category: &str) -> Vec<Note> {
    db.list_notes(&NoteQuery {
        category: Some(category.into()),
        ..Default::default()
    })
    .unwrap()
}

#[test]
fn filtering_by_category_returns_only_that_categorys_live_notes() {
    let mut db = db();
    let work = db
        .create_note(NewNote {
            content: "work note".into(),
            category: Some("work".into()),
            source: None,
        })
        .unwrap();
    db.create_note(NewNote {
        content: "idea note".into(),
        category: Some("ideas".into()),
        source: None,
    })
    .unwrap();
    db.create_note(note("uncategorised")).unwrap();
    db.trash_note(&work.id).unwrap();

    assert_eq!(contents(&by_category(&db, "work")), [] as [&str; 0]);
    assert_eq!(contents(&by_category(&db, "ideas")), ["idea note"]);
}

#[test]
fn category_filter_combines_with_status_later_and_text() {
    let mut db = db();
    let work_open = db
        .create_note(NewNote {
            content: "work to do".into(),
            category: Some("work".into()),
            source: None,
        })
        .unwrap();
    db.create_note(NewNote {
        content: "work to read".into(),
        category: Some("work".into()),
        source: None,
    })
    .unwrap();
    db.set_status(&work_open.id, Status::Completed).unwrap();

    let both = db
        .list_notes(&NoteQuery {
            category: Some("work".into()),
            statuses: vec![Status::Completed],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(contents(&both), ["work to do"]);

    let with_text = db
        .list_notes(&NoteQuery {
            category: Some("work".into()),
            text: "read".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(contents(&with_text), ["work to read"]);
}

#[test]
fn a_category_filter_is_cleaned_like_a_saved_category() {
    let mut db = db();
    db.create_note(NewNote {
        content: "launch steps".into(),
        category: Some("Posthaste launch".into()),
        source: None,
    })
    .unwrap();
    assert_eq!(
        contents(&by_category(&db, "  #Posthaste launch ")),
        ["launch steps"]
    );
}

#[test]
fn category_counts_order_by_use_and_skip_trashed() {
    let mut db = db();
    let mut make = |category: &str| {
        db.create_note(NewNote {
            content: "x".into(),
            category: Some(category.into()),
            source: None,
        })
        .unwrap()
    };
    let work = make("work");
    make("ideas");
    make("ideas");
    make("later");
    // Parked notes are still live, so they count too.
    let parked = make("later");
    db.set_later(&parked.id, true).unwrap();
    db.trash_note(&work.id).unwrap();

    assert_eq!(
        db.category_counts().unwrap(),
        [
            CategoryCount {
                name: "ideas".into(),
                count: 2
            },
            CategoryCount {
                name: "later".into(),
                count: 2
            },
        ]
    );
}

#[test]
fn a_category_whose_notes_are_all_trashed_is_not_counted() {
    let mut db = db();
    let note = db
        .create_note(NewNote {
            content: "x".into(),
            category: Some("gone".into()),
            source: None,
        })
        .unwrap();
    assert_eq!(db.category_counts().unwrap().len(), 1);
    db.trash_note(&note.id).unwrap();
    assert!(db.category_counts().unwrap().is_empty());
}

// ---- search ----------------------------------------------------------------

#[test]
fn search_matches_words_prefixes_and_all_terms() {
    let mut db = db();
    db.create_note_at(note("Investigate PostgreSQL indexing strategy"), 1_000)
        .unwrap();
    db.create_note_at(note("Read about index-only scans"), 2_000)
        .unwrap();
    db.create_note_at(note("Buy milk"), 3_000).unwrap();

    assert_eq!(search(&db, "postgresql").len(), 1);
    // prefix, case-insensitive, newest first
    assert_eq!(
        contents(&search(&db, "IND")),
        [
            "Read about index-only scans",
            "Investigate PostgreSQL indexing strategy"
        ]
    );
    // every term must match
    assert_eq!(search(&db, "index strat").len(), 1);
    assert!(search(&db, "index milk").is_empty());
    assert!(search(&db, "nothing-like-this").is_empty());
}

#[test]
fn search_ignores_diacritics_and_covers_category() {
    let mut db = db();
    db.create_note(NewNote {
        content: "Đọc tài liệu về chỉ mục".into(),
        category: Some("học tập".into()),
        source: None,
    })
    .unwrap();
    assert_eq!(search(&db, "tai lieu").len(), 1);
    assert_eq!(search(&db, "hoc").len(), 1);
}

#[test]
fn search_tolerates_fts_syntax_and_punctuation() {
    let mut db = db();
    db.create_note(note("use \"quotes\" AND (parens) -- c++ 100%"))
        .unwrap();
    // None of these may error, whatever the user types.
    for q in [
        "\"", "AND", "(", "c++", "NEAR(", "*", "a:b", "100%", "--", "^x",
    ] {
        let _ = search(&db, q);
    }
    assert_eq!(search(&db, "\"quotes\"").len(), 1);
    assert_eq!(search(&db, "100%").len(), 1);
    assert_eq!(search(&db, "--").len(), 1);
    assert!(search(&db, "%%").is_empty());
}

#[test]
fn search_follows_edits_and_includes_archived_notes() {
    let mut db = db();
    let n = db.create_note(note("alpha")).unwrap();
    db.update_note(&n.id, "beta", None).unwrap();
    assert!(search(&db, "alpha").is_empty());
    assert_eq!(search(&db, "beta").len(), 1);

    db.set_status(&n.id, Status::Archived).unwrap();
    assert_eq!(search(&db, "beta").len(), 1);
    let open_only = db
        .list_notes(&NoteQuery {
            text: "beta".into(),
            statuses: vec![Status::Inbox],
            ..Default::default()
        })
        .unwrap();
    assert!(open_only.is_empty());
}

#[test]
fn search_stays_fast_with_a_large_collection() {
    let mut db = db();
    for i in 0..20_000 {
        db.create_note_at(note(&format!("note number {i} about topic{}", i % 97)), i)
            .unwrap();
    }
    db.create_note_at(note("the needle in the haystack"), 99_999)
        .unwrap();
    let started = std::time::Instant::now();
    let hits = search(&db, "needle hay");
    let elapsed = started.elapsed();
    assert_eq!(hits.len(), 1);
    assert!(
        elapsed.as_millis() < 100,
        "search took {elapsed:?} over 20k notes"
    );
}

// ---- drafts ----------------------------------------------------------------

#[test]
fn draft_round_trips_and_overwrites() {
    let mut db = db();
    assert_eq!(db.load_draft(DraftRetention::Forever).unwrap(), None);
    db.save_draft_at("half a thou", None, 1_000).unwrap();
    db.save_draft_at("half a thought", Some("ideas"), 2_000)
        .unwrap();
    assert_eq!(
        db.load_draft(DraftRetention::Forever).unwrap(),
        Some(Draft {
            content: "half a thought".into(),
            category: Some("ideas".into()),
            updated_at: 2_000,
        })
    );
}

#[test]
fn saving_an_empty_draft_clears_it() {
    let mut db = db();
    db.save_draft("something", None).unwrap();
    db.save_draft("   ", None).unwrap();
    assert_eq!(db.load_draft(DraftRetention::Forever).unwrap(), None);
}

#[test]
fn draft_expires_according_to_retention() {
    const HOUR: i64 = 60 * 60 * 1000;
    let mut db = db();
    db.save_draft_at("stale", None, 0).unwrap();
    assert!(db
        .load_draft_at(DraftRetention::Day, 23 * HOUR)
        .unwrap()
        .is_some());
    assert!(db
        .load_draft_at(DraftRetention::Forever, 1_000 * HOUR)
        .unwrap()
        .is_some());
    assert!(db
        .load_draft_at(DraftRetention::Day, 25 * HOUR)
        .unwrap()
        .is_none());
}

#[test]
fn capturing_saves_the_note_and_clears_the_draft_together() {
    let mut db = db();
    db.save_draft("Investigate indexing", None).unwrap();
    let n = db.capture_note(note("Investigate indexing")).unwrap();
    assert_eq!(db.get_note(&n.id).unwrap().content, "Investigate indexing");
    assert_eq!(db.load_draft(DraftRetention::Forever).unwrap(), None);
}

#[test]
fn a_failed_capture_keeps_the_draft() {
    let mut db = db();
    db.save_draft("   draft survives   ", None).unwrap();
    assert!(db.capture_note(note("   ")).is_err());
    assert!(db.load_draft(DraftRetention::Forever).unwrap().is_some());
}

#[test]
fn draft_and_notes_survive_an_unclean_shutdown() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("posthaste.db");
    {
        let mut db = Db::open(&path).unwrap();
        db.create_note(note("saved before the crash")).unwrap();
        db.save_draft("typed but not saved", None).unwrap();
        // Simulate a crash: the connection is never closed cleanly.
        std::mem::forget(db);
    }
    let mut db = Db::open(&path).unwrap();
    assert_eq!(search(&db, "crash").len(), 1);
    assert_eq!(
        db.load_draft(DraftRetention::Forever)
            .unwrap()
            .unwrap()
            .content,
        "typed but not saved"
    );
}

// ---- settings --------------------------------------------------------------

#[test]
fn settings_default_then_round_trip() {
    let mut db = db();
    assert_eq!(db.load_settings().unwrap(), Settings::default());
    let s = Settings {
        capture_shortcut: "Alt+Space".into(),
        hide_after_save: false,
        ..Default::default()
    };
    db.save_settings(&s).unwrap();
    assert_eq!(db.load_settings().unwrap(), s);
}

#[test]
fn settings_tolerate_missing_unknown_and_corrupt_values() {
    let db = db();
    let set = |json: &str| {
        db.conn()
            .execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES ('settings', ?1)",
                [json],
            )
            .unwrap();
    };
    set(r#"{"theme":"dark","some_future_field":true}"#);
    let s = db.load_settings().unwrap();
    // The dead theme key is ignored; the rest is the default.
    assert_eq!(s.look_id, crate::look::DEFAULT_LOOK_ID);
    assert_eq!(s.capture_shortcut, DEFAULT_CAPTURE_SHORTCUT);

    set("not json at all");
    assert_eq!(db.load_settings().unwrap(), Settings::default());
}

// ---- export, import, backup --------------------------------------------------

#[test]
fn export_then_import_into_a_fresh_database_preserves_everything() {
    let mut a = db();
    let n1 = a
        .create_note_at(
            NewNote {
                content: "first\nwith two lines".into(),
                category: Some("work".into()),
                source: Some("Terminal".into()),
            },
            1_700_000_000_000,
        )
        .unwrap();
    let n2 = a.create_note_at(note("second"), 1_700_000_001_000).unwrap();
    let n2 = a
        .set_status_at(&n2.id, Status::Completed, 1_700_000_002_000)
        .unwrap();
    let trashed = a.create_note(note("trashed")).unwrap();
    a.trash_note(&trashed.id).unwrap();

    let json = a.export_json().unwrap();
    assert!(json.contains("2023-11-14T22:13:20Z"), "readable timestamps");
    assert!(!json.contains("trashed"));

    let mut b = db();
    let report = b.import_json(&json).unwrap();
    assert_eq!(
        report,
        ImportReport {
            imported: 2,
            skipped_existing: 0,
            skipped_invalid: 0,
            skipped_steps: 0
        }
    );
    assert_eq!(b.get_note(&n1.id).unwrap(), n1);
    assert_eq!(b.get_note(&n2.id).unwrap(), n2);
    assert_eq!(
        search(&b, "two lines").len(),
        1,
        "imported notes are indexed"
    );
}

#[test]
fn importing_twice_never_duplicates_or_overwrites() {
    let mut a = db();
    let n = a.create_note(note("original")).unwrap();
    let json = a.export_json().unwrap();
    a.update_note(&n.id, "edited after export", None).unwrap();

    let report = a.import_json(&json).unwrap();
    assert_eq!((report.imported, report.skipped_existing), (0, 1));
    assert_eq!(a.get_note(&n.id).unwrap().content, "edited after export");
}

#[test]
fn import_rejects_foreign_files_without_touching_data() {
    let mut db = db();
    db.create_note(note("mine")).unwrap();
    for bad in [
        "",
        "[]",
        r#"{"format":"something-else","version":1,"exported_at":"","notes":[]}"#,
        r#"{"format":"posthaste-export","version":99,"exported_at":"","notes":[]}"#,
    ] {
        assert!(matches!(
            db.import_json(bad),
            Err(CoreError::InvalidImport(_))
        ));
    }
    assert_eq!(db.info().unwrap().note_count, 1);
}

#[test]
fn import_skips_invalid_entries_and_fills_in_missing_fields() {
    let mut db = db();
    let json = r#"{
      "format": "posthaste-export", "version": 1, "exported_at": "",
      "notes": [
        {"content": "minimal"},
        {"content": "   "},
        {"content": "bad status", "status": "pinned"},
        {"content": "done", "status": "completed", "created_at": "2024-01-01T00:00:00Z"}
      ]
    }"#;
    let report = db.import_json(json).unwrap();
    assert_eq!((report.imported, report.skipped_invalid), (2, 2));
    let done = &search(&db, "done")[0];
    assert_eq!(done.status, Status::Completed);
    assert_eq!(done.completed_at, Some(done.created_at));
}

// ---- Markdown export ---------------------------------------------------------

fn tricky_notes(db: &mut Db) -> Vec<String> {
    let t = 1_700_000_000_000;
    let open = db
        .create_note_at(
            NewNote {
                content: "Send the slides\nto Anna -- today".into(),
                category: Some("work stuff".into()),
                source: Some("Slack --> DM".into()),
            },
            t,
        )
        .unwrap();
    db.set_details_at(
        &open.id,
        "First line\n\n- [ ] not a separate note\n## not a heading\n```\ncode\n```\n   indented",
        t + 1,
    )
    .unwrap();
    let later = db.create_note_at(note("Someday"), t + 2).unwrap();
    db.set_later_at(&later.id, true, t + 3).unwrap();
    let done = db.create_note_at(note("Book flights"), t + 4).unwrap();
    db.set_status_at(&done.id, Status::Completed, t + 5)
        .unwrap();
    let archived = db.create_note_at(note("Old idea"), t + 6).unwrap();
    db.set_status_at(&archived.id, Status::Archived, t + 7)
        .unwrap();
    vec![open.id, later.id, done.id, archived.id]
}

#[test]
fn a_markdown_export_reads_like_a_task_list() {
    let mut db = db();
    tricky_notes(&mut db);
    // UTC+7: 22:13 UTC on 14 Nov shows as 05:13 on 15 Nov.
    let md = db.export_markdown(7 * 60).unwrap();
    assert!(md.starts_with("# Posthaste notes\n"), "{md}");
    assert!(md.contains("· 4 notes"), "{md}");
    for heading in ["## To do", "## Later", "## Done", "## Archived"] {
        assert!(md.contains(heading), "{heading}\n{md}");
    }
    assert!(
        md.contains("- [ ] Send the slides\n  to Anna -- today\n"),
        "{md}"
    );
    assert!(md.contains("- [x] Book flights"), "{md}");
    assert!(md.contains("Added 15 Nov 2023 05:13"), "{md}");
    assert!(md.contains("`work stuff`"), "{md}");
    assert!(md.contains("  - [ ] not a separate note"), "{md}");
    // Nothing in a hidden comment can close it early.
    for line in md.lines().filter(|l| l.contains("<!-- posthaste {")) {
        assert_eq!(line.matches("--").count(), 2, "{line}");
    }
}

#[test]
fn a_markdown_export_imports_back_without_loss() {
    let mut a = db();
    let ids = tricky_notes(&mut a);
    let md = a.export_markdown(-300).unwrap();

    let mut b = db();
    let report = b.import_text(&md).unwrap();
    assert_eq!(
        report,
        ImportReport {
            imported: 4,
            skipped_existing: 0,
            skipped_invalid: 0,
            skipped_steps: 0
        }
    );
    for id in &ids {
        let (was, now) = (a.note_detail(id).unwrap(), b.note_detail(id).unwrap());
        assert_eq!(now.note, was.note);
        assert_eq!(now.details, was.details);
    }
    // Importing again adds nothing.
    assert_eq!(b.import_text(&md).unwrap().skipped_existing, 4);
}

#[test]
fn ticking_a_box_in_another_app_counts_as_done() {
    let mut a = db();
    let n = a.create_note(note("Call the bank")).unwrap();
    let md = a
        .export_markdown(0)
        .unwrap()
        .replace("- [ ] Call", "- [x] Call");
    let mut b = db();
    b.import_text(&md).unwrap();
    let got = b.get_note(&n.id).unwrap();
    assert_eq!(got.status, Status::Completed);
    assert!(got.completed_at.is_some());
}

#[test]
fn a_hand_typed_task_list_imports_too() {
    let mut db = db();
    let md = "# My list\n\nSome intro text.\n\n- [ ] Buy milk\n- [x] Pay rent\n\n  Paid by transfer.\n* [ ] Water plants\n- not a task\n\n## Archived\n\n- [ ] Learn Fortran\n";
    let report = db.import_text(md).unwrap();
    assert_eq!(report.imported, 4);
    assert_eq!(search(&db, "milk")[0].status, Status::Inbox);
    let rent = &search(&db, "rent")[0];
    assert_eq!(rent.status, Status::Completed);
    assert_eq!(
        db.note_detail(&rent.id).unwrap().details,
        "Paid by transfer."
    );
    assert_eq!(search(&db, "Fortran")[0].status, Status::Archived);
    assert!(search(&db, "not a task").is_empty());
}

#[test]
fn markdown_import_rejects_files_that_are_not_notes() {
    let mut db = db();
    for bad in [
        "",
        "# Just a document\n\nNo tasks here.",
        "<!-- posthaste-export version 99 -->\n- [ ] from the future",
    ] {
        assert!(
            matches!(db.import_text(bad), Err(CoreError::InvalidImport(_))),
            "{bad:?}"
        );
    }
    // An empty export of ours is fine.
    let empty = Db::open_in_memory().unwrap().export_markdown(0).unwrap();
    assert_eq!(db.import_text(&empty).unwrap().imported, 0);
}

#[test]
fn import_still_reads_json_exports() {
    let mut a = db();
    let n = a.create_note(note("from json")).unwrap();
    let json = a.export_json().unwrap();
    let mut b = db();
    assert_eq!(
        b.import_text(&format!("\u{feff}{json}")).unwrap().imported,
        1
    );
    assert_eq!(b.get_note(&n.id).unwrap(), a.get_note(&n.id).unwrap());
}

// ---- steps in exports ------------------------------------------------------------

/// A note holding `lines` as steps, the first `done` of them ticked.
fn note_with_step_lines(db: &mut Db, lines: &[&str], done: usize) -> Note {
    let n = db.create_note(note("a note with steps")).unwrap();
    let steps: Vec<NewStep> = lines
        .iter()
        .enumerate()
        .map(|(i, text)| NewStep {
            text: (*text).into(),
            done: i < done,
        })
        .collect();
    db.add_steps(&n.id, None, &steps).unwrap();
    db.get_note(&n.id).unwrap()
}

#[test]
fn markdown_round_trip_preserves_step_ids_order_and_done_state() {
    let mut a = db();
    let n = note_with_step_lines(&mut a, &["Update website", "Prepare screenshots"], 1);

    let md = a.export_markdown(0).unwrap();
    assert!(md.contains("<!-- posthaste:steps -->"), "steps are marked");
    assert!(md.contains("- [x] Update website"), "done state is written");
    assert!(
        md.contains("- [ ] Prepare screenshots"),
        "open state is written"
    );

    let mut b = db();
    b.import_text(&md).unwrap();
    let before = a.list_steps(&n.id).unwrap();
    let after = b
        .list_steps(&b.list_notes(&NoteQuery::default()).unwrap()[0].id)
        .unwrap();
    assert_eq!(before, after, "ids, order and done state survive");
}

#[test]
fn description_text_with_checkboxes_stays_description() {
    let mut a = db();
    let n = note_with_step_lines(&mut a, &["a real step"], 0);
    a.set_details(&n.id, "- [ ] not a step, just a note\n- [x] neither")
        .unwrap();

    let md = a.export_markdown(0).unwrap();
    let mut b = db();
    b.import_text(&md).unwrap();
    let imported = b.list_notes(&NoteQuery::default()).unwrap()[0].id.clone();
    assert_eq!(b.note_detail(&imported).unwrap().steps.len(), 1);
    assert_eq!(
        b.note_detail(&imported).unwrap().details,
        "- [ ] not a step, just a note\n- [x] neither"
    );
}

#[test]
fn a_hand_typed_marked_checklist_imports_as_steps() {
    // A list typed by hand inside the steps markers: no ids, no ranks, and
    // the checkboxes carry the done state.
    let hand_typed = concat!(
        "- [ ] a note\n",
        "  <!-- posthaste:steps -->\n",
        "  - [ ] first thing\n",
        "  - [x] second thing\n",
        "  <!-- /posthaste:steps -->\n",
    );
    let mut b = db();
    let report = b.import_text(hand_typed).unwrap();
    assert_eq!(report.imported, 1);
    let id = b.list_notes(&NoteQuery::default()).unwrap()[0].id.clone();
    let steps = b.list_steps(&id).unwrap();
    assert_eq!(steps.len(), 2, "the marked lines became steps");
    assert_eq!(steps[0].text, "first thing");
    assert!(!steps[0].done);
    assert_eq!(steps[1].text, "second thing");
    assert!(steps[1].done);
    // ids were generated, and the second step sorts after the first.
    assert!(!steps[0].id.is_empty() && !steps[1].id.is_empty());
    assert!(steps[0].rank < steps[1].rank);
}

#[test]
fn markdown_without_a_steps_block_imports_as_before() {
    // An export written by the version before steps existed.
    let old = "\
# Posthaste notes

Exported 14 Nov 2026 09:12 · 1 notes

## To do

- [ ] written before steps existed
  <!-- posthaste {\"id\":\"legacy-1\",\"status\":\"inbox\",\"created_at\":\"2026-11-14T02:12:00Z\",\"updated_at\":\"2026-11-14T02:12:00Z\"} -->
  `work` · Added 14 Nov 2026 09:12
  Some description.
";
    let mut b = db();
    let report = b.import_text(old).unwrap();
    assert_eq!(report.imported, 1);
    let id = b.list_notes(&NoteQuery::default()).unwrap()[0].id.clone();
    assert_eq!(
        b.get_note(&id).unwrap().content,
        "written before steps existed"
    );
    assert_eq!(b.list_steps(&id).unwrap(), [] as [Step; 0]);
    assert_eq!(b.note_detail(&id).unwrap().details, "Some description.");
}

#[test]
fn json_v1_files_still_import() {
    let v1 =
        "{\"format\":\"posthaste-export\",\"version\":1,\"exported_at\":\"2026-11-14T02:12:00Z\",\
         \"notes\":[{\"id\":\"old-1\",\"content\":\"from a v1 file\",\"status\":\"inbox\",\
         \"created_at\":\"2026-11-14T02:12:00Z\"}]}";
    let mut b = db();
    let report = b.import_text(v1).unwrap();
    assert_eq!(report.imported, 1);
    assert_eq!(b.get_note("old-1").unwrap().content, "from a v1 file");
}

#[test]
fn json_export_import_preserves_step_ids_and_deleted_steps() {
    let mut a = db();
    let n = note_with_step_lines(&mut a, &["kept", "gone"], 0);
    let steps = a.list_steps(&n.id).unwrap();
    a.remove_step(&steps[1].id).unwrap();

    let json = a.export_json().unwrap();
    let mut b = db();
    let report = b.import_text(&json).unwrap();
    assert_eq!(report.imported, 1);
    let id = b.list_notes(&NoteQuery::default()).unwrap()[0].id.clone();
    assert_eq!(b.list_steps(&id).unwrap(), a.list_steps(&n.id).unwrap());
    // The tombstoned step is preserved in the file, kept deleted on import.
    let tombstones: i64 = b
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM note_steps WHERE note_id = ?1 AND deleted_at IS NOT NULL",
            [id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(tombstones, 1);
}

#[test]
fn importing_a_note_twice_does_not_duplicate_its_steps() {
    let mut a = db();
    let n = note_with_step_lines(&mut a, &["once"], 0);
    let md = a.export_markdown(0).unwrap();

    let mut b = db();
    b.import_text(&md).unwrap();
    let report = b.import_text(&md).unwrap();
    let _ = &n;
    assert_eq!(report.skipped_existing, 1);
    let id = b.list_notes(&NoteQuery::default()).unwrap()[0].id.clone();
    assert_eq!(b.list_steps(&id).unwrap().len(), 1);
}

#[test]
fn searching_finds_a_note_by_one_of_its_steps() {
    let mut db = db();
    let n = note_with_step_lines(&mut db, &["update the website screenshots", "unrelated"], 0);
    db.create_note(note("a note without steps")).unwrap();
    assert_eq!(search(&db, "screenshots"), [db.get_note(&n.id).unwrap()]);
    assert_eq!(search(&db, "unrelated").len(), 1);
}

#[test]
fn a_deleted_step_no_longer_matches_search() {
    let mut db = db();
    let n = note_with_step_lines(&mut db, &["buy stamps"], 0);
    let step = db.list_steps(&n.id).unwrap().remove(0);
    assert_eq!(search(&db, "stamps").len(), 1);
    db.remove_step(&step.id).unwrap();
    assert!(search(&db, "stamps").is_empty());
}

#[test]
fn searching_a_step_word_with_diacritics_removed_still_matches() {
    let mut db = db();
    let _ = note_with_step_lines(&mut db, &["uộn cho bài học"], 0);
    assert_eq!(search(&db, "uon").len(), 1);
    assert_eq!(search(&db, "hoc").len(), 1);
}

#[test]
fn a_backup_keeps_steps_order_done_state_and_deletions() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("posthaste.db");
    let mut db = Db::open(&path).unwrap();
    let n = note_with_step_lines(&mut db, &["kept", "gone", "ticked"], 1);
    let steps = db.list_steps(&n.id).unwrap();
    db.remove_step(&steps[1].id).unwrap();
    let before = db.list_steps(&n.id).unwrap();

    let backup = dir.path().join("backup.db");
    db.backup_to(&backup).unwrap();
    let copy = Db::open(&backup).unwrap();
    assert_eq!(copy.list_steps(&n.id).unwrap(), before);
    let tombstones: i64 = copy
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM note_steps WHERE note_id = ?1 AND deleted_at IS NOT NULL",
            [&n.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(tombstones, 1);
}

#[test]
fn export_file_and_backup_can_be_reopened() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = Db::open(&dir.path().join("posthaste.db")).unwrap();
    db.create_note(note("backed up")).unwrap();

    let export = dir.path().join("export.json");
    assert_eq!(db.export_to(&export, ExportFormat::Json).unwrap(), 1);
    assert!(std::fs::read_to_string(&export)
        .unwrap()
        .contains("backed up"));
    assert!(!dir.path().join("export.json.part").exists());

    let backup = dir.path().join("backup.db");
    db.backup_to(&backup).unwrap();
    db.backup_to(&backup).unwrap(); // overwriting an existing backup works
    let copy = Db::open(&backup).unwrap();
    assert_eq!(search(&copy, "backed").len(), 1);
}

#[test]
fn clear_all_removes_notes_drafts_and_settings() {
    let mut db = db();
    db.create_note(note("secret")).unwrap();
    db.save_draft("draft", None).unwrap();
    db.save_settings(&Settings {
        capture_shortcut: "Alt+Space".into(),
        ..Default::default()
    })
    .unwrap();
    db.clear_all().unwrap();
    assert_eq!(db.info().unwrap().note_count, 0);
    assert!(search(&db, "secret").is_empty());
    assert_eq!(db.load_draft(DraftRetention::Forever).unwrap(), None);
    assert_eq!(db.load_settings().unwrap(), Settings::default());
}

// ---- migrations --------------------------------------------------------------

#[test]
fn a_fresh_database_is_at_the_latest_schema() {
    let db = db();
    assert_eq!(
        db.info().unwrap().schema_version,
        MIGRATIONS.last().unwrap().version
    );
}

#[test]
fn migration_versions_are_strictly_increasing_from_one() {
    for (i, m) in MIGRATIONS.iter().enumerate() {
        assert_eq!(m.version, i as i64 + 1);
    }
}

#[test]
fn reopening_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("posthaste.db");
    let id = {
        let mut db = Db::open(&path).unwrap();
        db.create_note(note("persisted")).unwrap().id
    };
    for _ in 0..3 {
        let db = Db::open(&path).unwrap();
        assert_eq!(db.get_note(&id).unwrap().content, "persisted");
    }
    // No upgrade happened, so no snapshot was written.
    assert!(std::fs::read_dir(dir.path()).unwrap().count() <= 3);
}

#[test]
fn upgrading_from_v1_keeps_notes_indexes_them_and_leaves_a_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("posthaste.db");
    {
        // A database as version 1 of the app would have left it.
        let mut conn = Connection::open(&path).unwrap();
        migrations::migrate_with(&mut conn, Some(&path), &MIGRATIONS[..1]).unwrap();
        conn.execute(
            "INSERT INTO notes (id, content, created_at, updated_at)
             VALUES ('legacy-1', 'written before search existed', 1, 1)",
            [],
        )
        .unwrap();
    }

    let db = Db::open(&path).unwrap();
    assert_eq!(
        db.info().unwrap().schema_version,
        MIGRATIONS.last().unwrap().version
    );
    assert_eq!(
        db.get_note("legacy-1").unwrap().content,
        "written before search existed"
    );
    assert_eq!(search(&db, "before search").len(), 1);

    let snapshot = migrations::pre_migration_backup_path(&path, 1);
    assert!(snapshot.exists());
    let old = Connection::open(&snapshot).unwrap();
    assert_eq!(migrations::schema_version(&old).unwrap(), 1);
    let n: i64 = old
        .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1);
}

#[test]
fn upgrading_from_v2_keeps_notes_and_adds_the_looks_table() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("posthaste.db");
    {
        // A database as version 2 of the app would have left it.
        let mut conn = Connection::open(&path).unwrap();
        migrations::migrate_with(&mut conn, Some(&path), &MIGRATIONS[..2]).unwrap();
        conn.execute(
            "INSERT INTO notes (id, content, created_at, updated_at)
             VALUES ('legacy-2', 'written before looks existed', 1, 1)",
            [],
        )
        .unwrap();
    }

    let db = Db::open(&path).unwrap();
    assert_eq!(
        db.info().unwrap().schema_version,
        MIGRATIONS.last().unwrap().version
    );
    assert_eq!(
        db.get_note("legacy-2").unwrap().content,
        "written before looks existed"
    );
    let tables: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'looks'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(tables, 1);
}

#[test]
fn a_failing_migration_rolls_back_completely() {
    let mut conn = Connection::open_in_memory().unwrap();
    migrations::migrate_with(&mut conn, None, &MIGRATIONS[..1]).unwrap();
    conn.execute(
        "INSERT INTO notes (id, content, created_at, updated_at) VALUES ('a', 'safe', 1, 1)",
        [],
    )
    .unwrap();

    let broken = [
        Migration {
            version: 1,
            sql: MIGRATIONS[0].sql,
        },
        Migration {
            version: 2,
            sql: "ALTER TABLE notes ADD COLUMN extra TEXT; DELETE FROM notes; SELECT * FROM no_such_table;",
        },
    ];
    let err = migrations::migrate_with(&mut conn, None, &broken).unwrap_err();
    assert!(matches!(err, CoreError::Migration { version: 2, .. }));

    assert_eq!(migrations::schema_version(&conn).unwrap(), 1);
    let content: String = conn
        .query_row("SELECT content FROM notes WHERE id = 'a'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(content, "safe");
    assert!(conn.prepare("SELECT extra FROM notes").is_err());
}

#[test]
fn a_database_from_a_newer_version_is_refused_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("posthaste.db");
    drop(Db::open(&path).unwrap());
    Connection::open(&path)
        .unwrap()
        .pragma_update(None, "user_version", 999)
        .unwrap();

    assert!(matches!(
        Db::open(&path),
        Err(CoreError::NewerSchema {
            found: 999,
            supported
        }) if supported == MIGRATIONS.last().unwrap().version
    ));
    let conn = Connection::open(&path).unwrap();
    assert_eq!(migrations::schema_version(&conn).unwrap(), 999);
}

#[test]
fn a_file_that_is_not_a_database_is_reported_not_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("posthaste.db");
    std::fs::write(&path, "this is definitely not sqlite").unwrap();
    assert!(Db::open(&path).is_err());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "this is definitely not sqlite"
    );
}

#[test]
fn the_database_rejects_rows_that_bypass_validation() {
    let db = db();
    let insert = |content: &str, status: &str| {
        db.conn().execute(
            "INSERT INTO notes (id, content, status, created_at, updated_at)
             VALUES (hex(randomblob(8)), ?1, ?2, 1, 1)",
            [content, status],
        )
    };
    assert!(insert("   ", "inbox").is_err());
    assert!(insert("fine", "pinned").is_err());
    assert!(insert("fine", "inbox").is_ok());
}

// ---- glance (the Today window) ----------------------------------------------

const DAY_FROM: i64 = 1_000_000;
const DAY_TO: i64 = DAY_FROM + 86_400_000;

fn glance(db: &Db) -> Glance {
    db.glance(DAY_FROM, DAY_TO).unwrap()
}

#[test]
fn glance_on_an_empty_database_is_empty() {
    let g = glance(&db());
    assert!(g.carried.is_empty());
    assert!(g.today.is_empty());
    assert_eq!((g.done, g.total), (0, 0));
}

#[test]
fn glance_carries_open_notes_from_before_today() {
    let mut db = db();
    let carried = db.create_note_at(note("carried"), DAY_FROM - 1).unwrap();
    let old_done = db.create_note_at(note("old done"), DAY_FROM - 1).unwrap();
    db.set_status_at(&old_done.id, Status::Completed, DAY_FROM - 1)
        .unwrap();

    let g = glance(&db);
    assert_eq!(contents(&g.carried), ["carried"]);
    assert_eq!(g.carried[0].id, carried.id);
    assert!(g.today.is_empty());
    assert_eq!((g.done, g.total), (0, 1));
}

#[test]
fn glance_includes_notes_created_today_open_and_done() {
    let mut db = db();
    let done_note = db
        .create_note_at(note("made today"), DAY_FROM + 100)
        .unwrap();
    db.set_status_at(&done_note.id, Status::Completed, DAY_FROM + 200)
        .unwrap();
    db.create_note_at(note("open today"), DAY_FROM + 300)
        .unwrap();

    let g = glance(&db);
    assert_eq!(contents(&g.today), ["open today", "made today"]);
    assert!(g.carried.is_empty());
    assert_eq!((g.done, g.total), (1, 2));
}

#[test]
fn glance_counts_old_notes_completed_today_as_done() {
    let mut db = db();
    let n = db
        .create_note_at(note("finished late"), DAY_FROM - 5000)
        .unwrap();
    db.set_status_at(&n.id, Status::Completed, DAY_FROM + 10)
        .unwrap();

    let g = glance(&db);
    assert_eq!(contents(&g.today), ["finished late"]);
    assert!(g.carried.is_empty());
    assert_eq!((g.done, g.total), (1, 1));
}

#[test]
fn glance_excludes_archived_and_trashed() {
    let mut db = db();
    let archived = db
        .create_note_at(note("archived today"), DAY_FROM + 100)
        .unwrap();
    db.set_status_at(&archived.id, Status::Archived, DAY_FROM + 150)
        .unwrap();
    let trashed = db
        .create_note_at(note("trashed today"), DAY_FROM + 200)
        .unwrap();
    db.trash_note(&trashed.id).unwrap();

    let g = glance(&db);
    assert!(g.carried.is_empty() && g.today.is_empty());
    assert_eq!((g.done, g.total), (0, 0));
}

#[test]
fn glance_respects_day_bounds() {
    let mut db = db();
    db.create_note_at(note("at from"), DAY_FROM).unwrap();
    db.create_note_at(note("at to"), DAY_TO).unwrap();

    let g = glance(&db);
    assert_eq!(contents(&g.today), ["at from"]);
    assert!(g.carried.is_empty());
    assert_eq!(g.total, 1);
}

#[test]
fn completing_a_carried_note_moves_it_to_today_and_counts_it() {
    let mut db = db();
    let n = db.create_note_at(note("carried"), DAY_FROM - 1).unwrap();
    assert_eq!(glance(&db).done, 0);

    db.set_status_at(&n.id, Status::Completed, DAY_FROM + 5_000)
        .unwrap();
    let g = glance(&db);
    assert!(g.carried.is_empty());
    assert_eq!(contents(&g.today), ["carried"]);
    assert_eq!((g.done, g.total), (1, 1));
}

#[test]
fn glance_caps_each_list_but_not_the_counts() {
    let mut db = db();
    for i in 0..60 {
        db.create_note_at(note(&format!("carried {i}")), DAY_FROM - 1 - i)
            .unwrap();
    }
    let g = glance(&db);
    assert_eq!(g.carried.len(), 50);
    assert_eq!(contents(&g.carried)[0], "carried 0");
    assert!(g.today.is_empty());
    assert_eq!(g.total, 60);
}

#[test]
fn settings_from_an_older_version_get_the_default_today_shortcut() {
    let db = db();
    db.conn()
        .execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('settings', ?1)",
            [r#"{"theme":"dark"}"#],
        )
        .unwrap();
    let s = db.load_settings().unwrap();
    assert_eq!(s.today_shortcut, DEFAULT_TODAY_SHORTCUT);
}

#[test]
fn today_shortcut_round_trips_through_save_and_load() {
    let mut db = db();
    let s = Settings {
        today_shortcut: "F18".into(),
        ..Default::default()
    };
    db.save_settings(&s).unwrap();
    assert_eq!(db.load_settings().unwrap().today_shortcut, "F18");
}

// ---- note details ------------------------------------------------------------

#[test]
fn details_round_trip_and_flag_the_note() {
    let mut db = db();
    let n = db.create_note(note("Prepare Q4 review")).unwrap();
    assert!(!n.has_details);
    let updated = db
        .set_details_at(&n.id, "agenda\r\n- numbers\n", 5_000)
        .unwrap();
    assert!(updated.has_details);
    assert_eq!(updated.updated_at, 5_000);
    let d = db.note_detail(&n.id).unwrap();
    assert_eq!(d.details, "agenda\n- numbers\n");
    assert_eq!(d.note, updated);
    assert!(d.attachments.is_empty());

    let cleared = db.set_details(&n.id, "   \n").unwrap();
    assert!(!cleared.has_details);
    assert_eq!(db.note_detail(&n.id).unwrap().details, "");
}

#[test]
fn details_of_a_missing_or_trashed_note_report_not_found() {
    let mut db = db();
    assert!(matches!(db.note_detail("nope"), Err(CoreError::NotFound)));
    assert!(matches!(
        db.set_details("nope", "x"),
        Err(CoreError::NotFound)
    ));
    let n = db.create_note(note("gone soon")).unwrap();
    db.trash_note(&n.id).unwrap();
    assert!(matches!(db.note_detail(&n.id), Err(CoreError::NotFound)));
    assert!(matches!(
        db.set_details(&n.id, "x"),
        Err(CoreError::NotFound)
    ));
}

#[test]
fn details_over_the_limit_are_refused_and_keep_the_old_text() {
    let mut db = db();
    let n = db.create_note(note("long")).unwrap();
    db.set_details(&n.id, "kept").unwrap();
    let huge = "a".repeat(MAX_DETAILS_CHARS + 1);
    let err = db.set_details(&n.id, &huge).unwrap_err();
    assert_eq!(err.code(), "details_too_long");
    assert_eq!(db.note_detail(&n.id).unwrap().details, "kept");
}

#[test]
fn details_are_searchable_and_follow_edits() {
    let mut db = db();
    let n = db.create_note(note("Fix login bug")).unwrap();
    db.set_details(&n.id, "stacktrace mentions tokio runtime")
        .unwrap();
    assert_eq!(contents(&search(&db, "tokio")), ["Fix login bug"]);
    db.update_note(&n.id, "Fix login crash", None).unwrap();
    assert_eq!(contents(&search(&db, "tokio")), ["Fix login crash"]);
    db.set_details(&n.id, "solved").unwrap();
    assert!(search(&db, "tokio").is_empty());
    assert_eq!(search(&db, "solved").len(), 1);
}

#[test]
fn attachments_are_stored_listed_counted_and_served() {
    let mut db = db();
    let n = db.create_note(note("with files")).unwrap();
    let png = db
        .add_attachment_at(
            &n.id,
            "/Users/me/Desktop/chart.PNG",
            "",
            b"\x89PNGdata",
            7_000,
        )
        .unwrap();
    assert_eq!(png.name, "chart.PNG");
    assert_eq!(png.mime, "image/png");
    assert_eq!(png.size, 8);
    let pasted = db
        .add_attachment(&n.id, "Pasted image", "image/png", b"img")
        .unwrap();
    assert_eq!(pasted.mime, "image/png");
    let odd = db.add_attachment(&n.id, "notes", "", b"x").unwrap();
    assert_eq!(odd.mime, "application/octet-stream");

    let listed = db.get_note(&n.id).unwrap();
    assert_eq!(listed.attachment_count, 3);
    assert!(listed.updated_at >= 7_000);
    let in_list = search(&db, "files");
    assert_eq!(in_list[0].attachment_count, 3);

    let d = db.note_detail(&n.id).unwrap();
    assert_eq!(d.attachments.len(), 3);
    assert_eq!(d.attachments[0], png);
    let (meta, bytes) = db.attachment_bytes(&png.id).unwrap();
    assert_eq!(meta, png);
    assert_eq!(bytes, b"\x89PNGdata");

    db.remove_attachment(&png.id).unwrap();
    assert_eq!(db.get_note(&n.id).unwrap().attachment_count, 2);
    assert!(matches!(
        db.attachment_bytes(&png.id),
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        db.remove_attachment(&png.id),
        Err(CoreError::NotFound)
    ));
}

#[test]
fn attachment_limits_refuse_without_storing_anything() {
    let mut db = db();
    let n = db.create_note(note("limits")).unwrap();
    let err = db
        .add_attachment(&n.id, "big.mov", "", &vec![0u8; MAX_ATTACHMENT_BYTES + 1])
        .unwrap_err();
    assert_eq!(err.code(), "attachment_too_large");
    assert!(err.to_string().starts_with("This file is 15.0 MB"));
    assert!(
        !err.to_string().contains("big.mov"),
        "errors never carry file names"
    );
    let err = db.add_attachment(&n.id, "empty.txt", "", b"").unwrap_err();
    assert_eq!(err.code(), "attachment_empty");
    assert_eq!(db.get_note(&n.id).unwrap().attachment_count, 0);

    db.add_attachment(&n.id, "max.bin", "", &vec![1u8; MAX_ATTACHMENT_BYTES])
        .unwrap();
    for i in 1..MAX_ATTACHMENTS_PER_NOTE {
        db.add_attachment(&n.id, &format!("f{i}.txt"), "", b"x")
            .unwrap();
    }
    let err = db
        .add_attachment(&n.id, "one-more.txt", "", b"x")
        .unwrap_err();
    assert_eq!(err.code(), "too_many_attachments");
    assert_eq!(
        db.get_note(&n.id).unwrap().attachment_count as usize,
        MAX_ATTACHMENTS_PER_NOTE
    );
    assert!(matches!(
        db.add_attachment("missing", "a.txt", "", b"x"),
        Err(CoreError::NotFound)
    ));
}

#[test]
fn long_file_names_are_cut_but_keep_their_extension() {
    let mut db = db();
    let n = db.create_note(note("names")).unwrap();
    let long = format!("{}.pdf", "report-".repeat(40));
    let a = db.add_attachment(&n.id, &long, "", b"%PDF").unwrap();
    assert!(a.name.chars().count() <= 120);
    assert!(a.name.ends_with(".pdf"));
    assert_eq!(a.mime, "application/pdf");
    let b = db.add_attachment(&n.id, "  \u{7}\n ", "", b"x").unwrap();
    assert_eq!(b.name, "file");
}

#[test]
fn trashed_notes_hide_their_files_and_purge_removes_them() {
    let mut db = db();
    let n = db.create_note(note("purge me")).unwrap();
    let a = db.add_attachment(&n.id, "a.txt", "", b"x").unwrap();
    db.trash_note_at(&n.id, 1_000).unwrap();
    assert!(matches!(
        db.attachment_bytes(&a.id),
        Err(CoreError::NotFound)
    ));
    db.restore_trashed_at(&n.id, 1_500).unwrap();
    assert!(db.attachment_bytes(&a.id).is_ok());

    db.trash_note_at(&n.id, 1_000).unwrap();
    assert_eq!(db.purge_trashed(2_000).unwrap(), 1);
    let left: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM attachments", [], |r| r.get(0))
        .unwrap();
    assert_eq!(left, 0);
}

#[test]
fn clear_all_removes_attachments() {
    let mut db = db();
    let n = db.create_note(note("x")).unwrap();
    db.add_attachment(&n.id, "a.txt", "", b"x").unwrap();
    db.clear_all().unwrap();
    let left: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM attachments", [], |r| r.get(0))
        .unwrap();
    assert_eq!(left, 0);
}

#[test]
fn export_carries_descriptions_but_not_files() {
    let mut a = db();
    let n = a.create_note(note("exported")).unwrap();
    a.set_details(&n.id, "the long version").unwrap();
    a.add_attachment(&n.id, "secret.txt", "", b"file body")
        .unwrap();
    let plain = a.create_note(note("plain")).unwrap();
    let json = a.export_json().unwrap();
    assert!(json.contains("the long version"));
    assert!(!json.contains("secret.txt"));
    assert!(!json.contains("file body"));

    let mut b = db();
    b.import_json(&json).unwrap();
    let d = b.note_detail(&n.id).unwrap();
    assert_eq!(d.details, "the long version");
    assert!(d.attachments.is_empty());
    assert!(!b.get_note(&plain.id).unwrap().has_details);
    assert_eq!(search(&b, "long version").len(), 1);
}

#[test]
fn backup_contains_attachments() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = Db::open(&dir.path().join("p.db")).unwrap();
    let n = db.create_note(note("backed up")).unwrap();
    let a = db.add_attachment(&n.id, "a.bin", "", b"bytes").unwrap();
    let dest = dir.path().join("copy.db");
    db.backup_to(&dest).unwrap();
    let copy = Db::open(&dest).unwrap();
    assert_eq!(copy.attachment_bytes(&a.id).unwrap().1, b"bytes");
}

#[test]
fn upgrading_from_v3_keeps_notes_searchable_and_adds_details() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("posthaste.db");
    {
        let mut conn = Connection::open(&path).unwrap();
        migrations::migrate_with(&mut conn, Some(&path), &MIGRATIONS[..3]).unwrap();
        conn.execute(
            "INSERT INTO notes (id, content, category, created_at, updated_at)
             VALUES ('legacy-3', 'written before details existed', 'work', 1, 1)",
            [],
        )
        .unwrap();
    }
    let mut db = Db::open(&path).unwrap();
    let n = db.get_note("legacy-3").unwrap();
    assert!(!n.has_details);
    assert_eq!(n.attachment_count, 0);
    assert_eq!(search(&db, "before details").len(), 1);
    assert_eq!(search(&db, "work").len(), 1);
    db.set_details("legacy-3", "added later").unwrap();
    assert_eq!(search(&db, "added later").len(), 1);
    db.add_attachment("legacy-3", "a.txt", "", b"x").unwrap();
}

// ---- later (parked notes) -----------------------------------------------------

fn later_only(db: &Db, later: bool) -> Vec<Note> {
    db.list_notes(&NoteQuery {
        statuses: vec![Status::Inbox],
        later: Some(later),
        ..Default::default()
    })
    .unwrap()
}

#[test]
fn later_note_leaves_glance_and_counts() {
    let mut db = db();
    let old = db.create_note_at(note("old open"), DAY_FROM - 10).unwrap();
    let today = db
        .create_note_at(note("today open"), DAY_FROM + 10)
        .unwrap();
    db.create_note_at(note("stays"), DAY_FROM - 5).unwrap();
    let parked = db.set_later_at(&old.id, true, DAY_FROM + 20).unwrap();
    assert_eq!(parked.later_at, Some(DAY_FROM + 20));
    db.set_later_at(&today.id, true, DAY_FROM + 21).unwrap();

    let g = glance(&db);
    assert_eq!(contents(&g.carried), ["stays"]);
    assert!(g.today.is_empty());
    assert_eq!((g.done, g.total), (0, 1));
    assert_eq!(
        db.counts(DAY_FROM, DAY_TO).unwrap(),
        Counts {
            inbox: 1,
            today: 0,
            later: 2,
            archive: 0,
            trash: 0
        }
    );
}

#[test]
fn later_note_returns_to_the_today_pool_when_brought_back() {
    let mut db = db();
    let n = db.create_note_at(note("parked"), DAY_FROM - 10).unwrap();
    db.set_later(&n.id, true).unwrap();
    let back = db.set_later(&n.id, false).unwrap();
    assert_eq!(back.later_at, None);
    assert_eq!(back.status, Status::Inbox);
    assert_eq!(contents(&glance(&db).carried), ["parked"]);
}

#[test]
fn sending_a_done_note_to_later_reopens_it() {
    let mut db = db();
    let n = db.create_note_at(note("was done"), DAY_FROM - 10).unwrap();
    db.set_status(&n.id, Status::Completed).unwrap();
    let parked = db.set_later(&n.id, true).unwrap();
    assert_eq!(parked.status, Status::Inbox);
    assert_eq!(parked.completed_at, None);
    assert!(parked.later_at.is_some());
}

#[test]
fn completing_or_archiving_a_later_note_clears_it() {
    let mut db = db();
    let a = db.create_note(note("a")).unwrap();
    let b = db.create_note(note("b")).unwrap();
    db.set_later(&a.id, true).unwrap();
    db.set_later(&b.id, true).unwrap();
    assert_eq!(
        db.set_status(&a.id, Status::Completed).unwrap().later_at,
        None
    );
    assert_eq!(
        db.set_status(&b.id, Status::Archived).unwrap().later_at,
        None
    );
    // Back to the inbox, still not parked.
    assert_eq!(db.set_status(&b.id, Status::Inbox).unwrap().later_at, None);
}

#[test]
fn later_survives_trash_and_restore() {
    let mut db = db();
    let n = db.create_note(note("parked")).unwrap();
    db.set_later_at(&n.id, true, 42).unwrap();
    db.trash_note(&n.id).unwrap();
    assert!(later_only(&db, true).is_empty());
    assert_eq!(db.restore_trashed(&n.id).unwrap().later_at, Some(42));
}

#[test]
fn list_notes_later_filter_splits_inbox_from_later() {
    let mut db = db();
    let a = db.create_note_at(note("parked"), 1).unwrap();
    db.create_note_at(note("open"), 2).unwrap();
    db.set_later(&a.id, true).unwrap();
    assert_eq!(contents(&later_only(&db, true)), ["parked"]);
    assert_eq!(contents(&later_only(&db, false)), ["open"]);
    // No filter: search finds both.
    assert_eq!(search(&db, "parked").len(), 1);
    assert_eq!(db.list_notes(&NoteQuery::default()).unwrap().len(), 2);
}

#[test]
fn setting_later_on_a_missing_note_reports_not_found() {
    let mut db = db();
    assert!(matches!(
        db.set_later("nope", true),
        Err(CoreError::NotFound)
    ));
}

#[test]
fn export_round_trips_later_and_old_files_import_as_not_later() {
    let mut a = db();
    let n = a.create_note_at(note("parked"), 1_700_000_000_000).unwrap();
    let n = a.set_later_at(&n.id, true, 1_700_000_001_000).unwrap();
    let json = a.export_json().unwrap();
    assert!(json.contains("later_at"));
    let mut b = db();
    b.import_json(&json).unwrap();
    assert_eq!(b.get_note(&n.id).unwrap(), n);

    let old = r#"{"format":"posthaste-export","version":1,"exported_at":"2026-01-01T00:00:00Z",
        "notes":[{"id":"x1","content":"from an old file"}]}"#;
    let mut c = db();
    c.import_json(old).unwrap();
    assert_eq!(c.get_note("x1").unwrap().later_at, None);
}

#[test]
fn upgrading_from_v4_leaves_every_note_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("posthaste.db");
    {
        let mut conn = Connection::open(&path).unwrap();
        migrations::migrate_with(&mut conn, Some(&path), &MIGRATIONS[..4]).unwrap();
        conn.execute(
            "INSERT INTO notes (id, content, created_at, updated_at)
             VALUES ('legacy-4', 'carried over from before', ?1, ?1)",
            [DAY_FROM - 10],
        )
        .unwrap();
    }
    let db = Db::open(&path).unwrap();
    assert_eq!(
        db.info().unwrap().schema_version,
        MIGRATIONS.last().unwrap().version
    );
    let n = db.get_note("legacy-4").unwrap();
    assert_eq!(n.later_at, None);
    assert_eq!(contents(&glance(&db).carried), ["carried over from before"]);
}

// ---- window size ----------------------------------------------------------------

fn store_settings(db: &Db, json: &str) {
    db.conn()
        .execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('settings', ?1)",
            [json],
        )
        .unwrap();
}

#[test]
fn settings_without_window_size_load_as_medium() {
    let db = db();
    store_settings(&db, r#"{"theme":"dark"}"#);
    assert_eq!(db.load_settings().unwrap().window_size, WindowSize::Medium);
}

#[test]
fn unknown_window_size_loads_as_medium_and_keeps_other_settings() {
    let db = db();
    store_settings(
        &db,
        r#"{"theme":"dark","window_size":"gigantic","capture_shortcut":"Alt+Space"}"#,
    );
    let s = db.load_settings().unwrap();
    assert_eq!(s.window_size, WindowSize::Medium);
    assert_eq!(s.capture_shortcut, "Alt+Space");
}

#[test]
fn window_size_round_trips() {
    let mut db = db();
    let s = Settings {
        window_size: WindowSize::Large,
        ..Settings::default()
    };
    db.save_settings(&s).unwrap();
    assert_eq!(db.load_settings().unwrap().window_size, WindowSize::Large);
    assert!(serde_json::to_string(&s)
        .unwrap()
        .contains(r#""window_size":"large""#));
}

#[test]
fn window_size_factors_are_ordered_and_medium_is_one() {
    assert_eq!(WindowSize::Medium.factor(), 1.0);
    assert!(WindowSize::Small.factor() < 1.0);
    assert!(WindowSize::Large.factor() > 1.0);
}

// ---- day marks and past days ---------------------------------------------

const D: i64 = 86_400_000;
/// Five consecutive days: Monday..Friday, each `D` long.
const MON: i64 = 10 * D;
const TUE: i64 = MON + D;
const WED: i64 = TUE + D;
const THU: i64 = WED + D;
const FRI: i64 = THU + D;

fn past(db: &Db, day: i64) -> Glance {
    // Viewed long after the day ended.
    db.glance_at(day, day + D, FRI + 10 * D).unwrap()
}

#[test]
fn day_marks_count_total_and_open_per_day() {
    let mut db = db();
    db.create_note_at(note("mon open"), MON + 1).unwrap();
    let done = db.create_note_at(note("mon done"), MON + 2).unwrap();
    db.set_status_at(&done.id, Status::Completed, TUE).unwrap();
    let parked = db.create_note_at(note("tue parked"), TUE + 1).unwrap();
    db.set_later_at(&parked.id, true, TUE + 2).unwrap();
    let gone = db.create_note_at(note("tue trashed"), TUE + 3).unwrap();
    db.trash_note_at(&gone.id, TUE + 4).unwrap();
    let arch = db.create_note_at(note("thu archived"), THU + 1).unwrap();
    db.set_status_at(&arch.id, Status::Archived, THU + 2)
        .unwrap();

    let marks = db.day_marks(&[MON, TUE, WED, THU, FRI]).unwrap();
    assert_eq!(
        marks,
        vec![
            DayMark { total: 2, open: 1 },
            DayMark { total: 1, open: 0 },
            DayMark { total: 0, open: 0 },
            DayMark { total: 1, open: 0 },
        ]
    );
}

#[test]
fn day_marks_with_fewer_than_two_edges_is_empty() {
    let db = db();
    assert!(db.day_marks(&[]).unwrap().is_empty());
    assert!(db.day_marks(&[MON]).unwrap().is_empty());
}

#[test]
fn glance_for_a_past_day_shows_the_plate_as_it_was() {
    let mut db = db();
    let n = db.create_note_at(note("written monday"), MON + 5).unwrap();
    db.set_status_at(&n.id, Status::Completed, WED + 5).unwrap();

    let tue = past(&db, TUE);
    assert_eq!(contents(&tue.carried), ["written monday"]);
    assert!(tue.today.is_empty());
    assert_eq!((tue.done, tue.total), (0, 1));

    let wed = past(&db, WED);
    assert!(wed.carried.is_empty());
    assert_eq!(contents(&wed.today), ["written monday"]);
    assert_eq!((wed.done, wed.total), (1, 1));

    let thu = past(&db, THU);
    assert!(thu.carried.is_empty() && thu.today.is_empty());
    assert_eq!((thu.done, thu.total), (0, 0));
}

#[test]
fn glance_for_a_past_day_keeps_notes_archived_or_parked_later() {
    let mut db = db();
    let late_arch = db.create_note_at(note("archived fri"), MON).unwrap();
    db.set_status_at(&late_arch.id, Status::Archived, FRI)
        .unwrap();
    let early_arch = db.create_note_at(note("archived tue"), MON + 1).unwrap();
    db.set_status_at(&early_arch.id, Status::Archived, TUE)
        .unwrap();
    let late_park = db.create_note_at(note("parked fri"), MON + 2).unwrap();
    db.set_later_at(&late_park.id, true, FRI).unwrap();
    let early_park = db.create_note_at(note("parked tue"), MON + 3).unwrap();
    db.set_later_at(&early_park.id, true, TUE).unwrap();

    let wed = past(&db, WED);
    assert_eq!(contents(&wed.carried), ["parked fri", "archived fri"]);
}

#[test]
fn glance_for_a_past_day_counts_match_rows() {
    let mut db = db();
    db.create_note_at(note("old open"), MON).unwrap();
    let a = db.create_note_at(note("wed done"), WED + 1).unwrap();
    db.set_status_at(&a.id, Status::Completed, WED + 2).unwrap();
    let b = db.create_note_at(note("wed done later"), WED + 3).unwrap();
    db.set_status_at(&b.id, Status::Completed, THU).unwrap();
    db.create_note_at(note("wed open"), WED + 4).unwrap();

    let g = past(&db, WED);
    assert_eq!(contents(&g.carried), ["old open"]);
    assert_eq!(
        contents(&g.today),
        ["wed open", "wed done later", "wed done"]
    );
    assert_eq!((g.done, g.total), (1, 4));
}

#[test]
fn glance_for_today_is_unchanged() {
    let mut db = db();
    db.create_note_at(note("carried"), WED - 1).unwrap();
    let p = db.create_note_at(note("parked"), WED + 1).unwrap();
    db.set_later_at(&p.id, true, WED + 2).unwrap();
    db.create_note_at(note("today"), WED + 3).unwrap();
    // `now` inside the day: the live rules, same as `glance`.
    assert_eq!(
        db.glance_at(WED, WED + D, WED + 10).unwrap(),
        db.glance(WED, WED + D).unwrap()
    );
}

// ---- trash ---------------------------------------------------------------

const KEEP: i64 = TRASH_RETENTION_MS;
const T0: i64 = 100 * D;

#[test]
fn trash_keeps_notes_for_thirty_days() {
    assert_eq!(TRASH_RETENTION_MS, 30 * 86_400_000);
}

#[test]
fn trashed_notes_list_newest_first_with_their_deletion_time() {
    let mut db = db();
    let a = db.create_note_at(note("first deleted"), T0).unwrap();
    let b = db.create_note_at(note("second deleted"), T0 + 1).unwrap();
    db.create_note_at(note("alive"), T0 + 2).unwrap();
    db.trash_note_at(&a.id, T0 + 10).unwrap();
    db.trash_note_at(&b.id, T0 + 20).unwrap();
    let list = db.list_trashed_at(T0 + 30).unwrap();
    assert_eq!(
        list.iter()
            .map(|t| (t.note.content.as_str(), t.deleted_at))
            .collect::<Vec<_>>(),
        [("second deleted", T0 + 20), ("first deleted", T0 + 10)]
    );
}

#[test]
fn trashed_notes_past_the_keep_time_are_hidden_and_not_restorable() {
    let mut db = db();
    let old = db.create_note_at(note("old"), T0).unwrap();
    let recent = db.create_note_at(note("recent"), T0).unwrap();
    db.trash_note_at(&old.id, T0).unwrap();
    db.trash_note_at(&recent.id, T0 + D).unwrap();
    let now = T0 + KEEP + 1;
    assert_eq!(
        contents(&notes_of(&db.list_trashed_at(now).unwrap())),
        ["recent"]
    );
    assert!(matches!(
        db.restore_trashed_at(&old.id, now),
        Err(CoreError::NotFound)
    ));
    assert_eq!(
        db.restore_trashed_at(&recent.id, now).unwrap().content,
        "recent"
    );
}

#[test]
fn restoring_returns_status_later_and_attachments_unchanged() {
    let mut db = db();
    let done = db.create_note_at(note("done"), T0).unwrap();
    db.set_status_at(&done.id, Status::Completed, T0 + 1)
        .unwrap();
    let parked = db.create_note_at(note("parked"), T0).unwrap();
    db.set_later_at(&parked.id, true, T0 + 2).unwrap();
    let filed = db.create_note_at(note("filed"), T0).unwrap();
    db.add_attachment_at(&filed.id, "a.txt", "text/plain", b"hi", T0 + 3)
        .unwrap();
    for n in [&done, &parked, &filed] {
        db.trash_note_at(&n.id, T0 + 5).unwrap();
    }
    let now = T0 + 6;
    let done = db.restore_trashed_at(&done.id, now).unwrap();
    assert_eq!(
        (done.status, done.completed_at),
        (Status::Completed, Some(T0 + 1))
    );
    assert_eq!(
        db.restore_trashed_at(&parked.id, now).unwrap().later_at,
        Some(T0 + 2)
    );
    db.restore_trashed_at(&filed.id, now).unwrap();
    assert_eq!(db.note_detail(&filed.id).unwrap().attachments.len(), 1);
}

#[test]
fn purge_note_removes_attachments_and_the_search_entry() {
    let mut db = db();
    let n = db.create_note_at(note("secret plan"), T0).unwrap();
    let att = db
        .add_attachment_at(&n.id, "a.txt", "text/plain", b"hi", T0)
        .unwrap();
    db.trash_note_at(&n.id, T0 + 1).unwrap();
    db.purge_note(&n.id).unwrap();
    assert!(db.list_trashed_at(T0 + 2).unwrap().is_empty());
    assert!(matches!(
        db.attachment_bytes(&att.id),
        Err(CoreError::NotFound)
    ));
    assert!(search(&db, "secret").is_empty());
    assert!(matches!(
        db.restore_trashed_at(&n.id, T0 + 2),
        Err(CoreError::NotFound)
    ));
}

#[test]
fn purge_note_on_a_live_or_missing_note_is_not_found() {
    let mut db = db();
    let n = db.create_note_at(note("alive"), T0).unwrap();
    assert!(matches!(db.purge_note(&n.id), Err(CoreError::NotFound)));
    assert!(matches!(db.purge_note("nope"), Err(CoreError::NotFound)));
    assert_eq!(db.get_note(&n.id).unwrap().content, "alive");
}

#[test]
fn empty_trash_removes_only_trashed_notes_and_reports_the_count() {
    let mut db = db();
    for c in ["a", "b"] {
        let n = db.create_note_at(note(c), T0).unwrap();
        db.trash_note_at(&n.id, T0 + 1).unwrap();
    }
    db.create_note_at(note("kept"), T0).unwrap();
    assert_eq!(db.empty_trash_at(T0 + 1, T0 + 2).unwrap(), 2);
    assert!(db.list_trashed_at(T0 + 2).unwrap().is_empty());
    assert_eq!(contents(&search(&db, "")), ["kept"]);
    assert_eq!(db.empty_trash_at(T0 + 1, T0 + 2).unwrap(), 0);
}

#[test]
fn empty_trash_spares_notes_deleted_after_the_list_was_shown() {
    let mut db = db();
    let listed = db.create_note_at(note("listed"), T0).unwrap();
    let later = db.create_note_at(note("deleted meanwhile"), T0).unwrap();
    db.trash_note_at(&listed.id, T0 + 1).unwrap();
    db.trash_note_at(&later.id, T0 + 5).unwrap();
    assert_eq!(db.empty_trash_at(T0 + 1, T0 + 6).unwrap(), 1);
    assert_eq!(
        contents(&notes_of(&db.list_trashed_at(T0 + 6).unwrap())),
        ["deleted meanwhile"]
    );
}

#[test]
fn empty_trash_counts_only_restorable_notes() {
    let mut db = db();
    let old = db.create_note_at(note("expired"), T0).unwrap();
    let fresh = db.create_note_at(note("fresh"), T0).unwrap();
    db.trash_note_at(&old.id, T0).unwrap();
    db.trash_note_at(&fresh.id, T0 + KEEP).unwrap();
    // What the dialog promised: one note. The expired one waits for the purge.
    assert_eq!(db.empty_trash_at(T0 + KEEP, T0 + KEEP + 1).unwrap(), 1);
}

#[test]
fn restore_trashed_respects_the_keep_time_and_rejects_live_or_missing_notes() {
    let mut db = db();
    let old = db.create_note_at(note("long gone"), T0).unwrap();
    db.trash_note_at(&old.id, T0).unwrap();
    assert!(matches!(
        db.restore_trashed(&old.id),
        Err(CoreError::NotFound)
    ));
    let live = db.create_note(note("alive")).unwrap();
    assert!(matches!(
        db.restore_trashed(&live.id),
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        db.restore_trashed("nope"),
        Err(CoreError::NotFound)
    ));
}

#[test]
fn counts_include_trash_and_ignore_expired() {
    let mut db = db();
    let a = db.create_note_at(note("a"), T0).unwrap();
    let b = db.create_note_at(note("b"), T0).unwrap();
    db.create_note_at(note("c"), T0).unwrap();
    db.trash_note_at(&a.id, T0).unwrap();
    db.trash_note_at(&b.id, T0 + 2 * D).unwrap();
    let c = db.counts_at(T0, T0 + D, T0 + KEEP + D).unwrap();
    assert_eq!((c.inbox, c.trash), (1, 1));
}

#[test]
fn trashed_notes_stay_out_of_every_other_read() {
    let mut db = db();
    let n = db.create_note_at(note("gone note"), T0 + 5).unwrap();
    db.trash_note_at(&n.id, T0 + 6).unwrap();
    assert!(search(&db, "gone").is_empty());
    assert!(db.list_notes(&NoteQuery::default()).unwrap().is_empty());
    let g = db.glance_at(T0, T0 + D, T0 + 10).unwrap();
    assert_eq!(g.total, 0);
    let g = db.glance_at(T0, T0 + D, T0 + 10 * D).unwrap();
    assert_eq!(g.total, 0);
    assert_eq!(db.day_marks(&[T0, T0 + D]).unwrap()[0].total, 0);
    let c = db.counts_at(T0, T0 + D, T0 + 10).unwrap();
    assert_eq!(
        (c.inbox, c.today, c.later, c.archive, c.trash),
        (0, 0, 0, 0, 1)
    );
    assert!(!db.export_json().unwrap().contains("gone note"));
}

#[test]
fn purge_trashed_uses_the_retention_cutoff() {
    let mut db = db();
    let now = T0 + 60 * D;
    let young = db.create_note_at(note("29 days"), T0).unwrap();
    let old = db.create_note_at(note("31 days"), T0).unwrap();
    db.trash_note_at(&young.id, now - 29 * D).unwrap();
    db.trash_note_at(&old.id, now - 31 * D).unwrap();
    assert_eq!(db.purge_trashed(now - TRASH_RETENTION_MS).unwrap(), 1);
    assert_eq!(
        contents(&notes_of(&db.list_trashed_at(now).unwrap())),
        ["29 days"]
    );
}

fn notes_of(list: &[TrashedNote]) -> Vec<Note> {
    list.iter().map(|t| t.note.clone()).collect()
}

#[test]
fn the_old_upper_position_loads_as_top() {
    let db = db();
    store_settings(&db, r#"{"capture_position":"upper"}"#);
    assert_eq!(
        db.load_settings().unwrap().capture_position,
        CapturePosition::Top
    );
}

#[test]
fn today_position_starts_where_capture_was() {
    // Before 0.3.2 Today shared the capture position; keep it where it was.
    let db = db();
    store_settings(&db, r#"{"capture_position":"upper"}"#);
    assert_eq!(
        db.load_settings().unwrap().today_position,
        CapturePosition::Top
    );
    let fresh = Db::open_in_memory().unwrap();
    assert_eq!(
        fresh.load_settings().unwrap().today_position,
        CapturePosition::Center
    );
}

#[test]
fn unknown_position_loads_as_center_and_keeps_other_settings() {
    let db = db();
    store_settings(
        &db,
        r#"{"theme":"dark","capture_position":"sideways","today_position":"nowhere"}"#,
    );
    let s = db.load_settings().unwrap();
    assert_eq!(s.capture_position, CapturePosition::Center);
    assert_eq!(s.today_position, CapturePosition::Center);
}

#[test]
fn every_position_round_trips_for_both_windows() {
    let all = [
        (CapturePosition::TopLeft, "top_left"),
        (CapturePosition::Top, "top"),
        (CapturePosition::TopRight, "top_right"),
        (CapturePosition::Left, "left"),
        (CapturePosition::Center, "center"),
        (CapturePosition::Right, "right"),
        (CapturePosition::BottomLeft, "bottom_left"),
        (CapturePosition::Bottom, "bottom"),
        (CapturePosition::BottomRight, "bottom_right"),
    ];
    for (position, name) in all {
        let mut db = db();
        let s = Settings {
            capture_position: position,
            today_position: CapturePosition::BottomRight,
            ..Settings::default()
        };
        db.save_settings(&s).unwrap();
        let loaded = db.load_settings().unwrap();
        assert_eq!(loaded.capture_position, position);
        assert_eq!(loaded.today_position, CapturePosition::BottomRight);
        assert!(serde_json::to_string(&s)
            .unwrap()
            .contains(&format!(r#""capture_position":"{name}""#)));
    }
}

// ---- AI providers --------------------------------------------------------

#[test]
fn old_ai_settings_load_into_the_per_provider_shape() {
    let db = db();
    store_settings(
        &db,
        r#"{"ai_provider":"openai","ai_api_key":"sk-old","ai_model":"gpt-4.1-nano"}"#,
    );
    let s = db.load_settings().unwrap();
    assert_eq!(s.ai_provider, "openai");
    assert_eq!(
        s.ai_models.get("openai").map(String::as_str),
        Some("gpt-4.1-nano")
    );
    // The old key is readable once, for moving into the keychain...
    assert_eq!(s.ai_api_key, "sk-old");
}

#[test]
fn a_provider_that_has_shut_down_reverts_to_off() {
    let db = db();
    store_settings(
        &db,
        r#"{"ai_provider":"github","ai_models":{"github":"openai/gpt-4.1","openai":"gpt-4o"},"ai_model":"x"}"#,
    );
    let s = db.load_settings().unwrap();
    assert_eq!(s.ai_provider, "none");
    assert!(!s.ai_models.contains_key("github"));
    assert_eq!(s.ai_models["openai"], "gpt-4o");
}

#[test]
fn an_old_key_stays_on_disk_until_it_has_moved_and_no_other_key_is_ever_written() {
    let mut db = db();
    let raw = |db: &Db| -> String {
        db.conn()
            .query_row(
                "SELECT value FROM settings WHERE key = 'settings'",
                [],
                |r| r.get(0),
            )
            .unwrap()
    };
    // Nothing to move: no key field at all, and the old model field is gone.
    let s = Settings {
        ai_provider: "anthropic".into(),
        ai_model: "legacy".into(),
        ..Settings::default()
    };
    db.save_settings(&s).unwrap();
    assert!(!raw(&db).contains("ai_api_key"), "{}", raw(&db));
    assert!(!raw(&db).contains("\"ai_model\""), "{}", raw(&db));

    // An old key not yet moved survives any save, so a failed move or an
    // early save can never lose it.
    let s = Settings {
        ai_api_key: "sk-old".into(),
        ..s
    };
    db.save_settings(&s).unwrap();
    assert_eq!(db.load_settings().unwrap().ai_api_key, "sk-old");
}

#[test]
fn any_provider_id_round_trips_with_its_model_and_address() {
    let mut db = db();
    let mut s = Settings {
        ai_provider: "custom".into(),
        ..Settings::default()
    };
    s.ai_models.insert("custom".into(), "my-model".into());
    s.ai_base_urls
        .insert("custom".into(), "http://localhost:1234/v1".into());
    db.save_settings(&s).unwrap();
    let loaded = db.load_settings().unwrap();
    assert_eq!(loaded.ai_provider, "custom");
    assert_eq!(loaded.ai_models["custom"], "my-model");
    assert_eq!(loaded.ai_base_urls["custom"], "http://localhost:1234/v1");
}

#[test]
fn ai_is_off_by_default() {
    assert_eq!(Settings::default().ai_provider, "none");
}

#[test]
fn fallback_secrets_are_stored_apart_from_the_settings() {
    let mut db = db();
    assert_eq!(db.secret_get("ai:openai").unwrap(), None);
    db.secret_set("ai:openai", "sk-1").unwrap();
    db.secret_set("ai:openai", "sk-2").unwrap();
    assert_eq!(db.secret_get("ai:openai").unwrap().as_deref(), Some("sk-2"));
    // Settings load and save untouched by a stored secret.
    db.save_settings(&Settings::default()).unwrap();
    assert_eq!(db.load_settings().unwrap(), Settings::default());
    db.secret_delete("ai:openai").unwrap();
    assert_eq!(db.secret_get("ai:openai").unwrap(), None);
    // Deleting what is not there is fine.
    db.secret_delete("ai:openai").unwrap();
}

#[test]
fn clearing_all_data_removes_fallback_secrets() {
    let mut db = db();
    db.secret_set("ai:gemini", "key").unwrap();
    db.clear_all().unwrap();
    assert_eq!(db.secret_get("ai:gemini").unwrap(), None);
}

#[test]
fn upgrading_from_v5_keeps_looks_and_adds_the_asset_table() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("posthaste.db");
    {
        // A database as version 5 of the app would have left it.
        let mut conn = Connection::open(&path).unwrap();
        migrations::migrate_with(&mut conn, Some(&path), &MIGRATIONS[..5]).unwrap();
        conn.execute(
            "INSERT INTO looks (id, doc, created_at, updated_at)
             VALUES ('user:legacy', '{\"format\":\"posthaste-look\",\"version\":1}', 1, 1)",
            [],
        )
        .unwrap();
    }
    let db = Db::open(&path).unwrap();
    assert_eq!(
        db.info().unwrap().schema_version,
        MIGRATIONS.last().unwrap().version
    );
    let n: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM looks", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1, "the stored look survived the upgrade");
    let a: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM look_assets", [], |r| r.get(0))
        .unwrap();
    assert_eq!(a, 0, "the new table exists and starts empty");
}

#[test]
fn upgrading_from_v11_keeps_notes_and_adds_query_indexes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("posthaste.db");
    {
        let mut conn = Connection::open(&path).unwrap();
        migrations::migrate_with(&mut conn, Some(&path), &MIGRATIONS[..11]).unwrap();
        conn.execute(
            "INSERT INTO notes (id, content, status, category, created_at, updated_at)
             VALUES ('n1', 'hello', 'inbox', 'Work', 1000, 1000)",
            [],
        )
        .unwrap();
    }
    let db = Db::open(&path).unwrap();
    assert_eq!(
        db.info().unwrap().schema_version,
        MIGRATIONS.last().unwrap().version
    );
    assert_eq!(db.projects(true).unwrap().len(), 1);
    assert_eq!(db.counts_at(0, 2000, 1500).unwrap().inbox, 1);
}
