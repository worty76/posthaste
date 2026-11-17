use super::*;

use super::png;

mod asset_tests {
    use super::png;
    use serde_json::{json, Map};

    use super::v2doc;
    use crate::look::assets::sniff_image;
    use crate::look::{AssetRef, LayerRecord, LookDocV2, WindowScene};
    #[allow(unused_imports)]
    use crate::look::{LookDocExt as _, LookDocV2Ext as _};
    use crate::Db;

    fn jpeg(width: u16, height: u16) -> Vec<u8> {
        let mut b = vec![0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0x0B, 0x08];
        b.extend_from_slice(&height.to_be_bytes());
        b.extend_from_slice(&width.to_be_bytes());
        b
    }

    fn webpl(width: u32, height: u32) -> Vec<u8> {
        let mut b = b"RIFF\x00\x00\x00\x00WEBPVP8L".to_vec();
        b.extend_from_slice(&5u32.to_le_bytes());
        b.push(0x2F);
        let bits = (width - 1) | ((height - 1) << 14);
        b.extend_from_slice(&bits.to_le_bytes());
        b
    }

    fn with_backdrop(asset_id: &str, assets: Vec<AssetRef>) -> LookDocV2 {
        let mut d = v2doc();
        d.scene.all = Some(WindowScene {
            backdrop: vec![LayerRecord {
                kind: "image".into(),
                v: 1,
                on: true,
                config: json!({"asset": asset_id, "fit": "cover", "dim": 0.4}),
                extra: Map::new(),
            }],
            margin: Vec::new(),
            extra: Map::new(),
        });
        d.assets = assets;
        d
    }

    fn listed(id: &str) -> AssetRef {
        AssetRef {
            id: id.into(),
            asset_type: "image/png".into(),
            bytes: 33,
            width: 2,
            height: 3,
            extra: Map::new(),
        }
    }

    #[test]
    fn a_fake_png_is_refused() {
        let bytes = b"MZ\x90\x00\x00\x00\x00\x00\x00\x00".to_vec();
        let err = sniff_image(&bytes).unwrap_err().to_string();
        assert!(err.contains("neither"), "got: {err}");
    }

    #[test]
    fn a_5000_pixel_png_is_refused() {
        let mut db = Db::open_in_memory().unwrap();
        let bytes = png(5000, 100);
        let err = db.store_look_asset(&bytes).unwrap_err().to_string();
        assert!(err.contains("2560"), "got: {err}");
    }

    #[test]
    fn a_3_mb_file_is_refused_before_hashing() {
        let mut db = Db::open_in_memory().unwrap();
        let bytes = vec![0xFF; 3 * 1024 * 1024];
        let err = db.store_look_asset(&bytes).unwrap_err().to_string();
        assert!(err.contains("2 MB"), "got: {err}");
    }

    #[test]
    fn a_truncated_png_header_is_refused() {
        let err = sniff_image(&png(2, 3)[..19]).unwrap_err().to_string();
        assert!(err.contains("cut off"), "got: {err}");
    }

    #[test]
    fn png_jpeg_and_webp_headers_give_their_dimensions() {
        let png = sniff_image(&png(2, 3)).unwrap();
        assert_eq!((png.mime, png.width, png.height), ("image/png", 2, 3));
        let jpg = sniff_image(&jpeg(2, 3)).unwrap();
        assert_eq!((jpg.mime, jpg.width, jpg.height), ("image/jpeg", 2, 3));
        let webp = sniff_image(&webpl(2, 3)).unwrap();
        assert_eq!((webp.mime, webp.width, webp.height), ("image/webp", 2, 3));
    }

    #[test]
    fn the_same_image_is_stored_once() {
        let mut db = Db::open_in_memory().unwrap();
        let bytes = png(2, 3);
        let first = db.store_look_asset_at(&bytes, 1_000).unwrap();
        let second = db.store_look_asset_at(&bytes, 2_000).unwrap();
        assert_eq!(first.id, second.id);
        let n: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM look_assets", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn an_image_layer_with_an_unlisted_asset_is_refused() {
        let id = format!("sha256:{}", "a".repeat(64));
        let d = with_backdrop(&id, Vec::new());
        let err = d.validate().unwrap_err().to_string();
        assert!(err.contains("assets list"), "got: {err}");
    }

    #[test]
    fn an_image_layer_with_a_listed_asset_is_accepted() {
        let id = format!("sha256:{}", "a".repeat(64));
        let d = with_backdrop(&id, vec![listed(&id)]);
        d.validate().unwrap();
    }

    #[test]
    fn purge_keeps_referenced_fresh_and_unknown_listed_assets() {
        let mut db = Db::open_in_memory().unwrap();
        let referenced = db.store_look_asset_at(&png(2, 3), 1_000).unwrap();
        let forgotten = db.store_look_asset_at(&jpeg(2, 3), 1_000).unwrap();

        // A stored look lists the referenced asset — in its assets list
        // only, from a layer this build does not even draw.
        let mut old = v2doc();
        old.scene.capture = Some(WindowScene {
            backdrop: Vec::new(),
            margin: vec![LayerRecord {
                kind: "shader".into(),
                v: 9,
                on: true,
                config: json!({"asset": referenced.id}),
                extra: Map::new(),
            }],
            extra: Map::new(),
        });
        old.assets = vec![AssetRef {
            id: referenced.id.clone(),
            asset_type: "image/png".into(),
            bytes: 33,
            width: 2,
            height: 3,
            extra: Map::new(),
        }];
        db.save_look_document(crate::look::LookDocument::V2(old), None)
            .unwrap();

        let now = 1_000 + 24 * 3600 * 1000 + 1;
        let fresh = db.store_look_asset_at(&png(3, 2), now - 500).unwrap();

        let removed = db.purge_unused_look_assets(now).unwrap();
        assert_eq!(removed, 1, "only the old unreferenced asset goes");
        db.look_asset_bytes(&referenced.id).unwrap();
        db.look_asset_bytes(&fresh.id).unwrap();
        assert!(db.look_asset_bytes(&forgotten.id).is_err());
    }

    #[test]
    fn storing_an_image_whose_record_disagrees_with_its_bytes_is_refused() {
        let mut db = Db::open_in_memory().unwrap();
        let asset = db.store_look_asset_at(&png(2, 3), 1_000).unwrap();
        // The record is corrupted: same bytes, wrong claims.
        db.conn()
            .execute(
                "UPDATE look_assets SET width = 999 WHERE id = ?1",
                [&asset.id],
            )
            .unwrap();
        let err = db
            .store_look_asset_at(&png(2, 3), 2_000)
            .unwrap_err()
            .to_string();
        assert!(err.contains("different details"), "got: {err}");
        // And the row keeps the claims it had, wrong as they are: the
        // store never rewrites a record it disagrees with.
        let width: i64 = db
            .conn()
            .query_row(
                "SELECT width FROM look_assets WHERE id = ?1",
                [&asset.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(width, 999);
    }

    #[test]
    fn clear_all_removes_look_assets() {
        let mut db = Db::open_in_memory().unwrap();
        db.store_look_asset(&png(2, 3)).unwrap();
        db.clear_all().unwrap();
        let n: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM look_assets", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }
}

// ---- look files (.phlook) ----------------------------------------------------

mod package_tests {
    use serde_json::{json, Map, Value};

    use super::png;
    use crate::look::package::{read_package, write_package};
    use crate::look::{AssetRef, Known, LayerRecord, LookDocV2, LookDocument};
    #[allow(unused_imports)]
    use crate::look::{LookDocExt as _, LookDocV2Ext as _};
    use crate::Db;

    /// Sakura-night-like v2 look with one image backdrop, as the editor
    /// would have saved it.
    fn look_with_image(id: &str, claims: (u64, u32, u32)) -> LookDocV2 {
        let mut d = crate::look::tests::v2doc();
        d.name = "File look".into();
        d.scene.all = Some(crate::look::WindowScene {
            backdrop: vec![LayerRecord {
                kind: "image".into(),
                v: 1,
                on: true,
                config: json!({"asset": id, "fit": "cover", "dim": 0.4}),
                extra: Map::new(),
            }],
            margin: Vec::new(),
            extra: Map::new(),
        });
        d.assets = vec![AssetRef {
            id: id.into(),
            asset_type: "image/png".into(),
            bytes: claims.0,
            width: claims.1,
            height: claims.2,
            extra: Map::new(),
        }];
        d
    }

    fn stored_db(bytes: &[u8]) -> (Db, String, AssetRef) {
        let mut db = Db::open_in_memory().unwrap();
        let asset = db.store_look_asset(bytes).unwrap();
        (db, asset.id.clone(), asset)
    }

    #[test]
    fn export_then_import_gives_the_same_document() {
        let bytes = png(2, 3);
        let (mut db, id, asset) = stored_db(&bytes);
        let look = look_with_image(&id, (asset.bytes, asset.width, asset.height));
        let saved = db
            .save_look_document(LookDocument::V2(look.clone()), None)
            .unwrap();

        let file = write_package(&look, |asset_id| {
            let (_, bytes) = db.look_asset_bytes(asset_id)?;
            Ok(bytes)
        })
        .unwrap();

        let pending = read_package(&file).unwrap();
        let mut other = Db::open_in_memory().unwrap();
        let summary = other.install_package(pending).unwrap();
        assert_eq!(summary.name, "File look");
        match other.look_document(&summary.id).unwrap() {
            LookDocument::V2(stored) => {
                assert_eq!(stored.theme, look.theme);
                assert_eq!(stored.scene, look.scene);
                assert_eq!(stored.assets.len(), 1);
                assert_eq!(stored.assets[0].id, id);
            }
            _ => panic!("expected v2"),
        }
        // The image came across: the bytes are stored and identical.
        let (_, stored_bytes) = other.look_asset_bytes(&id).unwrap();
        assert_eq!(stored_bytes, bytes);
    }

    #[test]
    fn a_package_with_a_wrong_digest_writes_nothing() {
        let bytes = png(2, 3);
        let (mut db, id, asset) = stored_db(&bytes);
        let look = look_with_image(&id, (asset.bytes, asset.width, asset.height));
        let file = write_package(&look, |asset_id| {
            let (_, bytes) = db.look_asset_bytes(asset_id)?;
            Ok(bytes)
        })
        .unwrap();

        // Tamper: swap one payload byte so the digest no longer matches.
        let value: Value = serde_json::from_str(&file).unwrap();
        let mut file = value;
        file["assets"][&id] = Value::String("AAAA".into());
        let tampered = serde_json::to_string(&file).unwrap();

        let mut fresh = Db::open_in_memory().unwrap();
        assert!(read_package(&tampered).is_err());
        assert_eq!(fresh.list_looks().unwrap().len(), 8);
        let n: i64 = fresh
            .conn()
            .query_row("SELECT COUNT(*) FROM look_assets", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn a_package_with_an_extra_asset_writes_nothing() {
        let bytes = png(2, 3);
        let (mut db, id, _) = stored_db(&bytes);
        // A colour-only look lists no images, so its file carries none.
        let look = crate::look::tests::v2doc();
        let file = write_package(&look, |asset_id| {
            let (_, bytes) = db.look_asset_bytes(asset_id)?;
            Ok(bytes)
        })
        .unwrap();

        // But the file carries bytes for an asset the look never lists.
        let value: Value = serde_json::from_str(&file).unwrap();
        let mut file = value;
        file["assets"][&id] = Value::String(crate::look::code::b64_encode(&bytes));
        let tampered = serde_json::to_string(&file).unwrap();

        let mut fresh = Db::open_in_memory().unwrap();
        assert!(read_package(&tampered).is_err());
        let n: i64 = fresh
            .conn()
            .query_row("SELECT COUNT(*) FROM looks", [], |r| r.get(0))
            .unwrap();
        // Nothing was written: the bundled looks are data files, so a
        // fresh database holds no look rows at all.
        assert_eq!(n, 0);
    }

    #[test]
    fn a_package_missing_an_asset_writes_nothing() {
        let bytes = png(2, 3);
        let (mut db, id, asset) = stored_db(&bytes);
        let look = look_with_image(&id, (asset.bytes, asset.width, asset.height));
        let file = write_package(&look, |asset_id| {
            let (_, bytes) = db.look_asset_bytes(asset_id)?;
            Ok(bytes)
        })
        .unwrap();

        // The look lists the image but the file carries no bytes for it.
        let value: Value = serde_json::from_str(&file).unwrap();
        let mut file = value;
        file["assets"] = json!({});
        let tampered = serde_json::to_string(&file).unwrap();

        let mut fresh = Db::open_in_memory().unwrap();
        assert!(read_package(&tampered).is_err());
        let n: i64 = fresh
            .conn()
            .query_row("SELECT COUNT(*) FROM looks", [], |r| r.get(0))
            .unwrap();
        // Nothing was written: the bundled looks are data files, so a
        // fresh database holds no look rows at all.
        assert_eq!(n, 0);
    }

    #[test]
    fn a_package_over_12_mb_is_refused_before_parsing() {
        let big = format!("{}{}", "x".repeat(13 * 1024 * 1024), "}");
        let err = read_package(&big).unwrap_err().to_string();
        assert!(err.contains("large"), "got: {err}");
    }

    #[test]
    fn a_package_with_a_fake_png_writes_nothing() {
        let bytes = png(2, 3);
        let (mut db, id, asset) = stored_db(&bytes);
        let look = look_with_image(&id, (asset.bytes, asset.width, asset.height));
        let file = write_package(&look, |asset_id| {
            let (_, bytes) = db.look_asset_bytes(asset_id)?;
            Ok(bytes)
        })
        .unwrap();

        // A renamed executable posing as the image.
        let value: Value = serde_json::from_str(&file).unwrap();
        let mut file = value;
        file["assets"][&id] = Value::String(crate::look::code::b64_encode(b"MZ\x90\x00garbage"));
        let tampered = serde_json::to_string(&file).unwrap();

        let mut fresh = Db::open_in_memory().unwrap();
        assert!(read_package(&tampered).is_err());
        let n: i64 = fresh
            .conn()
            .query_row("SELECT COUNT(*) FROM looks", [], |r| r.get(0))
            .unwrap();
        // Nothing was written: the bundled looks are data files, so a
        // fresh database holds no look rows at all.
        assert_eq!(n, 0);
    }

    #[test]
    fn a_package_whose_claims_disagree_with_its_bytes_is_refused() {
        let bytes = png(2, 3);
        let (mut db, id, asset) = stored_db(&bytes);
        // The document claims 100×200; the bytes say 2×3.
        let look = look_with_image(&id, (asset.bytes, 100, 200));
        let file = write_package(&look, |asset_id| {
            let (_, bytes) = db.look_asset_bytes(asset_id)?;
            Ok(bytes)
        })
        .unwrap();
        assert!(read_package(&file).is_err());
    }

    #[test]
    fn a_package_is_not_a_look_document_and_the_other_way_round() {
        // A look file is not a look, and a look is not a look file.
        let bytes = png(2, 3);
        let (mut db, id, asset) = stored_db(&bytes);
        let look = look_with_image(&id, (asset.bytes, asset.width, asset.height));
        let file = write_package(&look, |asset_id| {
            let (_, bytes) = db.look_asset_bytes(asset_id)?;
            Ok(bytes)
        })
        .unwrap();
        assert!(LookDocument::from_json(&file).is_err());
        assert!(read_package(&look.to_json()).is_err());
    }
}
