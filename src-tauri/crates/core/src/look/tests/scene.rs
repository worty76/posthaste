use super::*;

mod resolve_tests {
    use posthaste_model::{ResolvedLayer, Scene, WindowScene};

    use serde_json::{json, Map};

    use super::{asset_id, listed_asset, v2doc};
    use crate::look::resolve::resolve_scene;
    #[allow(unused_imports)]
    use crate::look::LookDocV2Ext as _;
    use crate::look::{LayerRecord, LookDocument};
    use crate::{Db, Settings};

    fn emoji(char: &str, anchor: &str) -> LayerRecord {
        LayerRecord {
            kind: "emoji".into(),
            v: 1,
            on: true,
            config: json!({"char": char, "anchor": anchor, "tilt": 0}),
            extra: Map::new(),
        }
    }

    fn image(dim: f64, asset: &str) -> LayerRecord {
        LayerRecord {
            kind: "image".into(),
            v: 1,
            on: true,
            config: json!({"asset": asset, "fit": "cover", "dim": dim}),
            extra: Map::new(),
        }
    }

    fn margin_scene(layers: Vec<LayerRecord>) -> WindowScene {
        WindowScene {
            backdrop: Vec::new(),
            margin: layers,
            extra: Map::new(),
        }
    }

    #[test]
    fn resolve_scene_puts_all_before_the_window_layers() {
        let mut d = v2doc();
        d.scene.all = Some(margin_scene(vec![emoji("🌸", "top-right")]));
        d.scene.capture = Some(margin_scene(vec![emoji("✨", "bottom-left")]));
        let resolved = resolve_scene(&d);
        let ResolvedLayer::Emoji { char, .. } = &resolved.capture.margin[0] else {
            panic!("expected a sticker");
        };
        assert_eq!(char, "🌸");
        let ResolvedLayer::Emoji { char, .. } = &resolved.capture.margin[1] else {
            panic!("expected a sticker");
        };
        assert_eq!(char, "✨");
        // `all` draws in every window, so main shows the all-sticker too.
        assert_eq!(resolved.main.margin.len(), 1);
        assert_eq!(resolved.skipped, 0);
    }

    #[test]
    fn resolve_scene_skips_and_counts_unknown_off_and_bad_layers() {
        let mut d = v2doc();
        let unknown = LayerRecord {
            kind: "shader".into(),
            v: 9,
            on: true,
            config: json!({"source": "spiral"}),
            extra: Map::new(),
        };
        let mut off = emoji("🌸", "top-left");
        off.on = false;
        let malformed = LayerRecord {
            kind: "emoji".into(),
            v: 1,
            on: true,
            config: json!({"char": "🌸", "anchor": "middle", "tilt": 0}),
            extra: Map::new(),
        };
        let failing = image(0.4, "sha256:missing");
        d.scene.capture = Some(margin_scene(vec![
            unknown,
            off,
            malformed,
            failing,
            emoji("✨", "bottom-right"),
        ]));
        let resolved = resolve_scene(&d);
        assert_eq!(resolved.capture.margin.len(), 1, "only the good sticker");
        assert_eq!(resolved.skipped, 4);
    }

    #[test]
    fn dim_is_rounded_to_tenths() {
        let mut d = v2doc();
        let listed = listed_asset(&asset_id());
        let id = listed.id.clone();
        d.assets = vec![listed];
        let mut third = image(0.4, &id);
        third.config.as_object_mut().unwrap().remove("dim");
        d.scene.all = Some(WindowScene {
            backdrop: vec![image(0.45, &id), image(0.44, &id), third],
            margin: Vec::new(),
            extra: Map::new(),
        });
        let resolved = resolve_scene(&d);
        let ResolvedLayer::Image { dim_tenths, .. } = &resolved.capture.backdrop[0] else {
            panic!("expected an image");
        };
        assert_eq!(*dim_tenths, 5);
        let ResolvedLayer::Image { dim_tenths, .. } = &resolved.capture.backdrop[1] else {
            panic!("expected an image");
        };
        assert_eq!(*dim_tenths, 4);
        let ResolvedLayer::Image { dim_tenths, .. } = &resolved.capture.backdrop[2] else {
            panic!("expected an image");
        };
        assert_eq!(*dim_tenths, 0);
    }

    #[test]
    fn simple_looks_gives_an_empty_active_scene() {
        let mut db = Db::open_in_memory().unwrap();
        let mut d = v2doc();
        d.scene.capture = Some(margin_scene(vec![emoji("🌸", "top-left")]));
        let saved = db.save_look_document(LookDocument::V2(d), None).unwrap();
        db.save_settings(&Settings {
            simple_looks: true,
            look_id: Some(saved.id),
            ..Settings::default()
        })
        .unwrap();
        let resolved = db.active_scene().unwrap();
        assert!(resolved.capture.margin.is_empty());
        assert!(resolved.capture.backdrop.is_empty());
    }

    #[test]
    fn without_simple_looks_the_active_scene_draws() {
        let mut db = Db::open_in_memory().unwrap();
        let mut d = v2doc();
        d.scene.capture = Some(margin_scene(vec![emoji("🌸", "top-left")]));
        let saved = db.save_look_document(LookDocument::V2(d), None).unwrap();
        db.save_settings(&Settings {
            look_id: Some(saved.id),
            ..Settings::default()
        })
        .unwrap();
        assert_eq!(db.active_scene().unwrap().capture.margin.len(), 1);
    }

    #[test]
    fn saving_with_a_scene_edit_keeps_unknown_layers_inside_it() {
        let mut db = Db::open_in_memory().unwrap();
        let id = db.save_look(super::doc(), None, None).unwrap().id;
        let edit = crate::look::SceneEdit {
            scene: Scene {
                capture: Some(margin_scene(vec![LayerRecord {
                    kind: "shader".into(),
                    v: 9,
                    on: true,
                    config: json!({"source": "spiral"}),
                    extra: Map::new(),
                }])),
                ..Scene::default()
            },
            assets: Vec::new(),
        };
        let mut edited = super::doc();
        edited.name = "Edited".into();
        db.save_look(edited, Some(edit), Some(&id)).unwrap();
        match db.look_document(&id).unwrap() {
            LookDocument::V2(stored) => {
                let layer = &stored.scene.capture.as_ref().unwrap().margin[0];
                assert_eq!(layer.kind, "shader");
                assert_eq!(layer.config, json!({"source": "spiral"}));
            }
            _ => panic!("expected v2"),
        }
    }

    #[test]
    fn saving_without_a_scene_keeps_the_stored_scene() {
        let mut db = Db::open_in_memory().unwrap();
        let id = db
            .save_look_document(
                LookDocument::V2(super::super::store::v2_store_tests::scene_doc()),
                None,
            )
            .unwrap()
            .id;
        let mut edited = super::doc();
        edited.name = "Edited".into();
        db.save_look(edited, None, Some(&id)).unwrap();
        match db.look_document(&id).unwrap() {
            LookDocument::V2(stored) => {
                assert!(
                    stored.scene.all.is_some(),
                    "the scene survived a sceneless save"
                );
            }
            _ => panic!("expected v2"),
        }
    }

    #[test]
    fn a_window_draws_at_most_six_stickers_and_the_own_ones_win() {
        let mut d = v2doc();
        d.scene.all = Some(margin_scene(vec![
            emoji("🌸", "top-left"),
            emoji("🌙", "top-right"),
            emoji("⭐", "bottom-left"),
            emoji("🍀", "bottom-right"),
        ]));
        d.scene.capture = Some(margin_scene(vec![
            emoji("🌷", "top-left"),
            emoji("🌻", "top-right"),
            emoji("🌼", "bottom-left"),
            emoji("🌺", "bottom-right"),
        ]));
        let resolved = resolve_scene(&d);
        // Six drawn: the window's own four first, then the first two of
        // all. The other two are counted, not drawn.
        assert_eq!(resolved.capture.margin.len(), 6);
        let chars: Vec<&str> = resolved
            .capture
            .margin
            .iter()
            .map(|layer| match layer {
                ResolvedLayer::Emoji { char, .. } => char.as_str(),
                _ => panic!("expected stickers"),
            })
            .collect();
        assert_eq!(chars, ["🌸", "🌙", "🌷", "🌻", "🌼", "🌺"]);
        assert_eq!(resolved.skipped, 2);
        // A window with no entry of its own still sees all four of all.
        assert_eq!(resolved.main.margin.len(), 4);
    }

    #[test]
    fn look_scene_of_a_v1_look_is_empty() {
        let mut db = Db::open_in_memory().unwrap();
        let id = db.save_look(super::doc(), None, None).unwrap().id;
        let edit = db.look_scene(&id).unwrap();
        assert!(edit.scene.is_empty());
        assert!(edit.assets.is_empty());
    }
}
