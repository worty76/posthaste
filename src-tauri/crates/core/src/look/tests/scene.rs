use super::*;

mod resolve_tests {
    use posthaste_model::{ResolvedLayer, Scene, WindowScene};

    use serde_json::{json, Map};

    use super::{asset_id, doc, listed_asset};
    use crate::look::resolve::resolve_parts;
    use crate::look::LayerRecord;
    #[allow(unused_imports)]
    use crate::look::LookDocExt as _;
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
    fn resolve_parts_puts_all_before_the_window_layers() {
        let mut d = doc();
        d.scene.all = Some(margin_scene(vec![emoji("🌸", "top-right")]));
        d.scene.capture = Some(margin_scene(vec![emoji("✨", "bottom-left")]));
        // The mechanics are resolve_parts’ job; the live gate is
        // tested separately, where stickers are left out unshipped.
        let resolved = resolve_parts(&d.scene, &d.assets);
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
    fn resolve_parts_skips_and_counts_unknown_off_and_bad_layers() {
        let mut d = doc();
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
        // The mechanics are resolve_parts’ job; the live gate is
        // tested separately, where stickers are left out unshipped.
        let resolved = resolve_parts(&d.scene, &d.assets);
        assert_eq!(resolved.capture.margin.len(), 1, "only the good sticker");
        assert_eq!(resolved.skipped, 4);
    }

    #[test]
    fn dim_is_rounded_to_tenths() {
        let mut d = doc();
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
        let resolved = resolve_parts(&d.scene, &d.assets);
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
        let mut d = doc();
        d.scene.capture = Some(margin_scene(vec![emoji("🌸", "top-left")]));
        let saved = db.save_look_document(d, None).unwrap();
        db.save_settings(&Settings {
            simple_looks: true,
            look_id: saved.id,
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
        let mut d = doc();
        let listed = listed_asset(&asset_id());
        let id = listed.id.clone();
        d.assets = vec![listed];
        d.scene.capture = Some(WindowScene {
            backdrop: vec![image(0.4, &id)],
            margin: Vec::new(),
            extra: Map::new(),
        });
        let saved = db.save_look_document(d, None).unwrap();
        db.save_settings(&Settings {
            look_id: saved.id,
            ..Settings::default()
        })
        .unwrap();
        assert_eq!(db.active_scene().unwrap().capture.backdrop.len(), 1);
    }

    #[test]
    fn saving_with_a_scene_edit_keeps_unknown_layers_inside_it() {
        let mut db = Db::open_in_memory().unwrap();
        let id = db.save_look(doc().theme_edit(), None, None).unwrap().id;
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
        let mut edited = doc();
        edited.name = "Edited".into();
        db.save_look(edited.theme_edit(), Some(edit), Some(&id))
            .unwrap();
        let stored = db.look_document(&id).unwrap();
        let layer = &stored.scene.capture.as_ref().unwrap().margin[0];
        assert_eq!(layer.kind, "shader");
        assert_eq!(layer.config, json!({"source": "spiral"}));
    }

    #[test]
    fn saving_without_a_scene_keeps_the_stored_scene() {
        let mut db = Db::open_in_memory().unwrap();
        let mut with_scene = doc();
        with_scene.scene.all = Some(margin_scene(vec![emoji("🌸", "top-right")]));
        let id = db.save_look_document(with_scene, None).unwrap().id;
        let mut edited = doc();
        edited.name = "Edited".into();
        db.save_look(edited.theme_edit(), None, Some(&id)).unwrap();
        let stored = db.look_document(&id).unwrap();
        assert!(
            stored.scene.all.is_some(),
            "the scene survived a sceneless save"
        );
    }

    #[test]
    fn a_window_draws_at_most_six_stickers_and_the_own_ones_win() {
        let mut d = doc();
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
        // The mechanics are resolve_parts’ job; the live gate is
        // tested separately, where stickers are left out unshipped.
        let resolved = resolve_parts(&d.scene, &d.assets);
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
    fn resolve_scene_leaves_stickers_out_while_they_are_unshipped() {
        let mut d = doc();
        d.scene.all = Some(margin_scene(vec![emoji("🌸", "top-right")]));
        // The live entry (what the app draws) leaves them out quietly.
        let resolved = crate::look::resolve::resolve_scene(&d);
        assert!(resolved.main.margin.is_empty());
        assert!(resolved.capture.margin.is_empty());
        // The mechanics underneath stay honest: resolve_parts itself
        // still places stickers, which is what ships them later.
        let ungated = crate::look::resolve::resolve_parts(&d.scene, &d.assets);
        assert_eq!(ungated.main.margin.len(), 1);
    }

    #[test]
    fn a_look_without_a_scene_answers_an_empty_edit() {
        let mut db = Db::open_in_memory().unwrap();
        let id = db.save_look(doc().theme_edit(), None, None).unwrap().id;
        let edit = db.look_scene(&id).unwrap();
        assert!(edit.scene.is_empty());
        assert!(edit.assets.is_empty());
    }
}
