use super::*;

mod store_tests {
    use super::doc;
    use crate::error::CoreError;
    use crate::look::code::encode_code;
    #[allow(unused_imports)]
    use crate::look::LookDocExt as _;
    use crate::{Db, Settings};

    /// The Strawberry Milk look with a different name, so saved looks are
    /// distinguishable in the list.
    fn named(name: &str) -> crate::look::LookDoc {
        let mut d = doc();
        d.name = name.into();
        d
    }

    #[test]
    fn a_fresh_database_lists_the_eight_bundled_looks() {
        let db = Db::open_in_memory().unwrap();
        let looks = db.list_looks().unwrap();
        assert_eq!(looks.len(), 8);
        assert!(looks.iter().all(|l| l.builtin));
        // Ink, the factory default, leads the gallery.
        assert_eq!(looks[0].id, crate::look::DEFAULT_LOOK_ID);
        assert_eq!(looks[1].id, "builtin:strawberry-milk");
    }

    #[test]
    fn saved_looks_get_user_ids_and_list_after_the_bundled_ones_newest_first() {
        let mut db = Db::open_in_memory().unwrap();
        let a = db.save_look_at(named("Alpha"), None, None, 1_000).unwrap();
        let b = db.save_look_at(named("Beta"), None, None, 2_000).unwrap();
        assert!(a.id.starts_with("user:"));
        assert!(b.id.starts_with("user:"));
        assert_ne!(a.id, b.id);

        let ids: Vec<String> = db.list_looks().unwrap().into_iter().map(|l| l.id).collect();
        assert_eq!(ids.len(), 10);
        assert_eq!(ids[8], b.id);
        assert_eq!(ids[9], a.id);
    }

    #[test]
    fn the_summary_preview_is_normalised() {
        let mut db = Db::open_in_memory().unwrap();
        let mut d = named("Upper");
        d.colors.bg = "#FFF".into();
        let s = db.save_look(d, None, None).unwrap();
        assert!(!s.builtin);
        assert_eq!(s.preview.bg, "#ffffff");
    }

    #[test]
    fn saving_with_an_existing_id_updates_in_place() {
        let mut db = Db::open_in_memory().unwrap();
        let s = db.save_look_at(named("First"), None, None, 1_000).unwrap();
        let updated = db
            .save_look_at(named("Second"), None, Some(&s.id), 2_000)
            .unwrap();
        assert_eq!(updated.id, s.id);
        assert_eq!(updated.name, "Second");
        let (created, updated_at): (i64, i64) = db
            .conn()
            .query_row(
                "SELECT created_at, updated_at FROM looks WHERE id = ?1",
                [&s.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((created, updated_at), (1_000, 2_000));
        assert_eq!(db.list_looks().unwrap().len(), 9);
    }

    #[test]
    fn saving_over_an_unreadable_stored_look_fails_and_keeps_the_row() {
        let mut db = Db::open_in_memory().unwrap();
        let broken = r#"{"version":2,"garbage":"#;
        db.conn()
            .execute(
                "INSERT INTO looks (id, doc, created_at, updated_at) VALUES ('user:broken', ?1, 1, 1)",
                [broken],
            )
            .unwrap();
        let result = db.save_look(named("New"), None, Some("user:broken"));
        assert!(result.is_err(), "the save must be refused");
        let doc: String = db
            .conn()
            .query_row("SELECT doc FROM looks WHERE id = 'user:broken'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(doc, broken, "the row must be left untouched");
    }

    #[test]
    fn updating_an_unknown_or_builtin_look_is_refused() {
        let mut db = Db::open_in_memory().unwrap();
        let d = named("X");
        assert!(matches!(
            db.save_look(d.clone(), None, Some("user:missing")),
            Err(CoreError::NotFound)
        ));
        assert!(matches!(
            db.save_look(d, None, Some("builtin:ink")),
            Err(CoreError::BuiltinLook)
        ));
    }

    #[test]
    fn an_invalid_document_is_rejected_and_the_list_is_unchanged() {
        let mut db = Db::open_in_memory().unwrap();
        let mut d = named("Bad");
        d.colors.text = d.colors.bg.clone();
        assert!(matches!(
            db.save_look(d, None, None),
            Err(CoreError::InvalidLook(_))
        ));
        assert_eq!(db.list_looks().unwrap().len(), 8);
    }

    #[test]
    fn the_fifty_look_limit_leaves_updates_alone() {
        let mut db = Db::open_in_memory().unwrap();
        let mut last = String::new();
        for i in 0..50 {
            last = db
                .save_look_at(named(&format!("Look {i}")), None, None, i as i64)
                .unwrap()
                .id;
        }
        assert!(matches!(
            db.save_look_at(named("One too many"), None, None, 100),
            Err(CoreError::TooManyLooks)
        ));
        db.save_look_at(named("Renamed"), None, Some(&last), 200)
            .unwrap();
        assert_eq!(db.list_looks().unwrap().len(), 58);
    }

    #[test]
    fn deleting_removes_user_looks_and_refuses_builtins() {
        let mut db = Db::open_in_memory().unwrap();
        let s = db.save_look(named("Doomed"), None, None).unwrap();
        db.delete_look(&s.id).unwrap();
        assert!(matches!(db.look_doc(&s.id), Err(CoreError::NotFound)));
        assert!(matches!(
            db.delete_look("builtin:ink"),
            Err(CoreError::BuiltinLook)
        ));
        assert!(matches!(
            db.delete_look("user:gone"),
            Err(CoreError::NotFound)
        ));
    }

    #[test]
    fn the_active_look_follows_settings_and_falls_back_to_ink() {
        let mut db = Db::open_in_memory().unwrap();
        // Ink is the factory default look.
        let active = db.active_look().unwrap().expect("factory default resolves");
        assert_eq!(active.id, crate::look::DEFAULT_LOOK_ID);

        // A look that cannot be found lands on Ink, not on Plain.
        db.save_settings(&Settings {
            look_id: Some("user:missing".into()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(
            db.active_look().unwrap().map(|l| l.id),
            Some(crate::look::DEFAULT_LOOK_ID.into())
        );

        // Plain (no look) is a choice the user made, and is kept.
        db.save_settings(&Settings {
            look_id: None,
            ..Default::default()
        })
        .unwrap();
        assert_eq!(db.active_look().unwrap(), None);

        let s = db.save_look(named("Mine"), None, None).unwrap();
        db.save_settings(&Settings {
            look_id: Some(s.id.clone()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(db.active_look().unwrap().map(|l| l.id), Some(s.id.clone()));
        db.delete_look(&s.id).unwrap();
        assert_eq!(
            db.active_look().unwrap().map(|l| l.id),
            Some(crate::look::DEFAULT_LOOK_ID.into())
        );
    }

    #[test]
    fn a_corrupt_stored_row_is_skipped_without_breaking_the_list() {
        let mut db = Db::open_in_memory().unwrap();
        db.save_look(named("Good"), None, None).unwrap();
        db.conn()
            .execute(
                "INSERT INTO looks (id, doc, created_at, updated_at)
                 VALUES ('user:broken', '{not json', 1, 1)",
                [],
            )
            .unwrap();
        let looks = db.list_looks().unwrap();
        assert_eq!(looks.len(), 9);
        assert!(looks.iter().all(|l| l.id != "user:broken"));

        db.save_settings(&Settings {
            look_id: Some("user:broken".into()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(
            db.active_look().unwrap().map(|l| l.id),
            Some(crate::look::DEFAULT_LOOK_ID.into())
        );
    }

    #[test]
    fn importing_a_code_stores_the_look_and_garbage_stores_nothing() {
        let mut db = Db::open_in_memory().unwrap();
        let s = db.import_look(&encode_code(&doc())).unwrap();
        assert!(s.id.starts_with("user:"));
        assert_eq!(s.name, "Strawberry Milk");
        assert!(matches!(
            db.import_look("hello"),
            Err(CoreError::InvalidLook(_))
        ));
        assert_eq!(db.list_looks().unwrap().len(), 9);
    }

    #[test]
    fn clear_all_removes_user_looks() {
        let mut db = Db::open_in_memory().unwrap();
        db.save_look(named("Mine"), None, None).unwrap();
        db.clear_all().unwrap();
        assert_eq!(db.list_looks().unwrap().len(), 8);
    }

    #[test]
    fn settings_default_to_ink_and_older_settings_adopt_it() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(
            db.load_settings().unwrap().look_id,
            Some(crate::look::DEFAULT_LOOK_ID.into())
        );
        // Settings saved before looks existed adopt the factory default…
        db.conn()
            .execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES ('settings', '{\"theme\":\"dark\"}')",
                [],
            )
            .unwrap();
        assert_eq!(
            db.load_settings().unwrap().look_id,
            Some(crate::look::DEFAULT_LOOK_ID.into())
        );
        // …while an explicit "no look" (the user picked Plain) is kept.
        db.conn()
            .execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES ('settings', '{\"theme\":\"dark\",\"look_id\":null}')",
                [],
            )
            .unwrap();
        assert_eq!(db.load_settings().unwrap().look_id, None);
    }
}

pub(super) mod v2_store_tests {
    use serde_json::{json, Map};

    use super::{doc, v2doc};
    use crate::look::code::{decode_code, encode_code, encode_code_v2, encode_document};
    use crate::look::derive::resolve;
    use crate::look::{LayerRecord, LookDocV2, LookDocument, WindowScene};
    #[allow(unused_imports)]
    use crate::look::{LookDocExt as _, LookDocV2Ext as _};
    use crate::Db;

    /// A look written as if by a future build: an unknown top-level field,
    /// an unknown layer kind, and an unknown key inside a known layer.
    pub(crate) const FUTURE_LOOK: &str = r##"{
      "format": "posthaste-look",
      "version": 2,
      "name": "Future build",
      "author": "Someone newer",
      "theme": {
        "base": "light",
        "colors": { "bg": "#fff5f7", "surface": "#ffffff", "text": "#4a2c3a", "accent": "#ff8fb1", "border": "#999999" },
        "shape": "round",
        "font": "rounded",
        "futureTheme": { "that": "stays" }
      },
      "scene": {
        "all": { "backdrop": [ { "kind": "image", "v": 1, "config": { "asset": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "fit": "cover", "dim": 0.45, "futureKey": "keep" } } ] },
        "capture": { "margin": [ { "kind": "shader", "v": 9, "on": false, "config": { "source": "spiral" }, "custom": "keep" } ] },
        "laterWindow": { "backdrop": [ { "kind": "mystery", "v": 1 } ] }
      },
      "assets": [ { "id": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "type": "image/png", "bytes": 412000, "width": 1920, "height": 1080, "future": true } ],
      "futureTop": { "still": "here" }
    }"##;

    fn scene_with_layers() -> WindowScene {
        WindowScene {
            backdrop: vec![LayerRecord {
                kind: "image".into(),
                v: 1,
                on: true,
                config: json!({"asset": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "dim": 0.45}),
                extra: Map::new(),
            }],
            margin: Vec::new(),
            extra: Map::new(),
        }
    }

    /// A valid v2 document: the scene plus its listed asset.
    pub(crate) fn scene_doc() -> LookDocV2 {
        let mut d = v2doc();
        d.scene.all = Some(scene_with_layers());
        d.assets = vec![super::listed_asset(&super::asset_id())];
        d
    }

    #[test]
    fn a_future_look_survives_parse_edit_save_export_and_reimport() {
        let mut db = Db::open_in_memory().unwrap();
        let mut parsed = LookDocV2::from_json(FUTURE_LOOK).unwrap();
        // The edit an editor makes: one colour.
        parsed.theme.colors.bg = "#eef2f7".into();
        parsed.validate().unwrap();

        // Save, export, and import into a fresh database.
        let saved = db
            .save_look_document(LookDocument::V2(parsed.clone()), None)
            .unwrap();
        let code = encode_document(&db.look_document(&saved.id).unwrap());
        let mut other = Db::open_in_memory().unwrap();
        let imported = other.import_look(&code).unwrap();
        let back = match other.look_document(&imported.id).unwrap() {
            LookDocument::V2(d) => d,
            _ => panic!("expected a v2 document"),
        };

        // The edited colour came through, and the whole document is
        // otherwise identical: every unknown part keeps its content.
        assert_eq!(back, parsed);
        assert_eq!(back.theme.colors.bg, "#eef2f7");
        assert_eq!(
            back.theme.colors.extra.get("border"),
            Some(&json!("#999999"))
        );
        assert_eq!(back.extra.get("futureTop"), Some(&json!({"still": "here"})));
        assert_eq!(
            back.theme.extra.get("futureTheme"),
            Some(&json!({"that": "stays"}))
        );
        let shader = &back.scene.capture.as_ref().unwrap().margin[0];
        assert_eq!(shader.kind, "shader");
        assert!(!shader.on);
        assert_eq!(shader.extra.get("custom"), Some(&json!("keep")));
        let image = &back.scene.all.as_ref().unwrap().backdrop[0];
        assert_eq!(image.config.get("futureKey"), Some(&json!("keep")));
        assert_eq!(back.assets[0].extra.get("future"), Some(&true.into()));
    }

    #[test]
    fn saving_a_v1_look_stores_a_v2_document_that_resolves_identically() {
        let mut db = Db::open_in_memory().unwrap();
        let s = db.save_look(doc(), None, None).unwrap();
        let stored = match db.look_document(&s.id).unwrap() {
            LookDocument::V2(d) => d,
            _ => panic!("expected the stored copy to be v2"),
        };
        assert_eq!(
            resolve(&s.id, &doc()).unwrap(),
            resolve(&s.id, &stored.v1_view()).unwrap()
        );
    }

    #[test]
    fn editing_an_imported_v2_look_from_the_editor_keeps_its_scene() {
        let d = scene_doc();
        let mut db = Db::open_in_memory().unwrap();
        let imported = db.import_look(&encode_code_v2(&d)).unwrap();

        // The editor only knows the v1 view: it renames and saves back.
        let mut edited = db.look_doc(&imported.id).unwrap();
        edited.name = "Renamed".into();
        db.save_look(edited, None, Some(&imported.id)).unwrap();

        let back = match db.look_document(&imported.id).unwrap() {
            LookDocument::V2(d) => d,
            _ => panic!("expected v2"),
        };
        assert_eq!(back.name, "Renamed");
        assert_eq!(back.scene.all.as_ref().unwrap().backdrop[0].kind, "image");
    }

    #[test]
    fn updating_a_stored_v1_look_migrates_it_to_v2() {
        let mut db = Db::open_in_memory().unwrap();
        db.conn()
            .execute(
                "INSERT INTO looks (id, doc, created_at, updated_at) VALUES ('user:v1', ?1, 1, 1)",
                [doc().to_json()],
            )
            .unwrap();
        let mut edited = doc();
        edited.name = "Updated".into();
        db.save_look_at(edited, None, Some("user:v1"), 2).unwrap();
        match db.look_document("user:v1").unwrap() {
            LookDocument::V2(d) => assert_eq!(d.name, "Updated"),
            _ => panic!("expected the save to migrate the row to v2"),
        }
    }

    #[test]
    fn a_stored_v1_row_still_lists_and_resolves_until_its_next_save() {
        let db = Db::open_in_memory().unwrap();
        db.conn()
            .execute(
                "INSERT INTO looks (id, doc, created_at, updated_at) VALUES ('user:v1', ?1, 1, 1)",
                [doc().to_json()],
            )
            .unwrap();
        let looks = db.list_looks().unwrap();
        assert!(looks.iter().any(|l| l.id == "user:v1"));
        assert!(matches!(
            db.look_document("user:v1").unwrap(),
            LookDocument::V1(_)
        ));
        assert_eq!(
            resolve("user:v1", &db.look_doc("user:v1").unwrap()).unwrap(),
            resolve("user:v1", &doc()).unwrap()
        );
    }

    #[test]
    fn importing_a_v1_code_stores_v2_and_a_v2_code_stays_v2() {
        let mut db = Db::open_in_memory().unwrap();
        let from_v1 = db.import_look(&encode_code(&doc())).unwrap();
        assert!(matches!(
            db.look_document(&from_v1.id).unwrap(),
            LookDocument::V2(_)
        ));
        let d = scene_doc();
        let from_v2 = db.import_look(&encode_code_v2(&d)).unwrap();
        match db.look_document(&from_v2.id).unwrap() {
            LookDocument::V2(stored) => assert!(stored.scene.all.is_some()),
            _ => panic!("expected v2"),
        }
        // Both resolve to the same tokens as the v1 original.
        assert_eq!(
            resolve(&from_v1.id, &doc()).unwrap().tokens,
            resolve(&from_v2.id, &v2doc().v1_view()).unwrap().tokens
        );
    }

    #[test]
    fn a_cut_v2_code_reports_incomplete() {
        let d = scene_doc();
        let code = encode_code_v2(&d);
        let cut = &code[..code.len() - 10];
        let err = decode_code(cut).unwrap_err().to_string();
        assert!(err.contains("incomplete"), "got: {err}");
    }

    #[test]
    fn a_document_with_a_v2_scene_survives_a_decode_cycle_verbatim() {
        let raw =
            decode_code(&encode_code_v2(&LookDocV2::from_json(FUTURE_LOOK).unwrap())).unwrap();
        let original = LookDocument::V2(LookDocV2::from_json(FUTURE_LOOK).unwrap());
        assert_eq!(raw, original);
    }
}
