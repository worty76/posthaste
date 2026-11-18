//! Storage tests: the gallery, saving, limits, deletion, the active
//! look, and what the editor's save keeps.

use super::{asset_id, doc, listed_asset, theme, OLD_VERSION};
use crate::look::LookDoc;

/// A Strawberry-Milk-shaped document with another name.
fn named(name: &str) -> LookDoc {
    let mut d = doc();
    d.name = name.into();
    d
}

mod one_format_store_tests {
    use serde_json::{json, Map};

    use super::{asset_id, doc, listed_asset, theme, OLD_VERSION};
    use crate::{Db, Settings};

    #[test]
    fn a_stored_version_1_row_is_skipped() {
        let mut db = Db::open_in_memory().unwrap();
        let row = json!({
            "format": "posthaste-look",
            "version": OLD_VERSION,
            "name": "Old",
            "base": "light",
            "colors": {
                "bg": "#ffffff", "surface": "#f2f2f2",
                "text": "#111111", "accent": "#3366cc"
            }
        });
        db.conn()
            .execute(
                "INSERT INTO looks (id, doc, created_at, updated_at)
                 VALUES ('user:v1', ?1, 1, 1)",
                [&row.to_string()],
            )
            .unwrap();
        let looks = db.list_looks().unwrap();
        assert_eq!(looks.len(), 8, "the eight bundled looks only");
        assert!(looks.iter().all(|l| l.id != "user:v1"));
        db.save_settings(&Settings {
            look_id: Some("user:v1".into()),
            ..Settings::default()
        })
        .unwrap();
        assert_eq!(
            db.active_look().unwrap().map(|l| l.id),
            Some(crate::look::DEFAULT_LOOK_ID.into())
        );
    }

    #[test]
    fn an_editor_save_keeps_unknown_theme_and_colour_fields() {
        let mut db = Db::open_in_memory().unwrap();
        let mut d = doc();
        d.theme.extra.insert("glow".into(), serde_json::json!(1));
        d.theme
            .colors
            .extra
            .insert("link".into(), serde_json::json!("#123456"));
        let saved = db.save_look_document(d, None).unwrap();

        let mut edit = db.look_theme(&saved.id).unwrap();
        edit.colors.accent = "#3366cc".into();
        db.save_look(edit, None, Some(&saved.id)).unwrap();

        let stored: String = db
            .conn()
            .query_row("SELECT doc FROM looks WHERE id = ?1", [&saved.id], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(stored.contains(r#""glow":1"#), "theme.extra kept: {stored}");
        assert!(
            stored.contains(r##""link":"#123456""##),
            "colors.extra kept: {stored}"
        );
        assert!(
            stored.contains("#3366cc"),
            "the new accent landed: {stored}"
        );
    }

    #[test]
    fn look_theme_answers_the_edit_the_editor_sends() {
        let mut db = Db::open_in_memory().unwrap();
        let id = db.save_look(theme(), None, None).unwrap().id;
        let edit = db.look_theme(&id).unwrap();
        assert_eq!(edit, theme());
        assert!(edit.font_family.is_none());
    }

    #[test]
    fn a_scene_and_its_assets_survive_an_editor_save() {
        use crate::look::{LayerRecord, WindowScene};

        let mut db = Db::open_in_memory().unwrap();
        let mut d = doc();
        d.scene.all = Some(WindowScene {
            backdrop: vec![LayerRecord {
                kind: "image".into(),
                v: 1,
                on: true,
                config: json!({"asset": asset_id(), "dim": 0.45}),
                extra: Map::new(),
            }],
            margin: Vec::new(),
            extra: Map::new(),
        });
        d.assets = vec![listed_asset(&asset_id())];
        let saved = db.save_look_document(d, None).unwrap();

        let mut edit = db.look_theme(&saved.id).unwrap();
        edit.name = "Renamed".into();
        db.save_look(edit, None, Some(&saved.id)).unwrap();

        let stored = db.look_document(&saved.id).unwrap();
        assert_eq!(stored.name, "Renamed");
        assert_eq!(stored.scene.all.as_ref().unwrap().backdrop.len(), 1);
        assert_eq!(stored.assets.len(), 1);
    }
}

mod store_tests {
    use super::{doc, named};
    use crate::error::CoreError;
    use crate::look::code::encode_code;
    #[allow(unused_imports)]
    use crate::look::LookDocExt as _;
    use crate::{Db, Settings};

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
        let a = db
            .save_look_at(named("Alpha").theme_edit(), None, None, 1_000)
            .unwrap();
        let b = db
            .save_look_at(named("Beta").theme_edit(), None, None, 2_000)
            .unwrap();
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
        d.theme.colors.bg = "#FFF".into();
        let s = db.save_look(d.theme_edit(), None, None).unwrap();
        assert!(!s.builtin);
        assert_eq!(s.preview.bg, "#ffffff");
    }

    #[test]
    fn saving_with_an_existing_id_updates_in_place() {
        let mut db = Db::open_in_memory().unwrap();
        let s = db
            .save_look_at(named("First").theme_edit(), None, None, 1_000)
            .unwrap();
        let updated = db
            .save_look_at(named("Second").theme_edit(), None, Some(&s.id), 2_000)
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
        let result = db.save_look(named("New").theme_edit(), None, Some("user:broken"));
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
        let edit = named("X").theme_edit();
        assert!(matches!(
            db.save_look(edit.clone(), None, Some("user:missing")),
            Err(CoreError::NotFound)
        ));
        assert!(matches!(
            db.save_look(edit, None, Some("builtin:ink")),
            Err(CoreError::BuiltinLook)
        ));
    }

    #[test]
    fn an_invalid_document_is_rejected_and_the_list_is_unchanged() {
        let mut db = Db::open_in_memory().unwrap();
        let mut edit = named("Bad").theme_edit();
        edit.colors.text = edit.colors.bg.clone();
        assert!(matches!(
            db.save_look(edit, None, None),
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
                .save_look_at(
                    named(&format!("Look {i}")).theme_edit(),
                    None,
                    None,
                    i as i64,
                )
                .unwrap()
                .id;
        }
        assert!(matches!(
            db.save_look_at(named("One too many").theme_edit(), None, None, 100),
            Err(CoreError::TooManyLooks)
        ));
        db.save_look_at(named("Renamed").theme_edit(), None, Some(&last), 200)
            .unwrap();
        assert_eq!(db.list_looks().unwrap().len(), 58);
    }

    #[test]
    fn deleting_removes_user_looks_and_refuses_builtins() {
        let mut db = Db::open_in_memory().unwrap();
        let s = db
            .save_look(named("Doomed").theme_edit(), None, None)
            .unwrap();
        db.delete_look(&s.id).unwrap();
        assert!(matches!(db.look_document(&s.id), Err(CoreError::NotFound)));
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

        let s = db
            .save_look(named("Mine").theme_edit(), None, None)
            .unwrap();
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
        db.save_look(named("Good").theme_edit(), None, None)
            .unwrap();
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
        db.save_look(named("Mine").theme_edit(), None, None)
            .unwrap();
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
