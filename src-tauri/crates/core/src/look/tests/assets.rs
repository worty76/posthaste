use super::*;

mod asset_tests {
    use serde_json::{json, Map};

    use super::v2doc;
    use crate::look::assets::sniff_image;
    use crate::look::{AssetRef, LayerRecord, LookDocV2, WindowScene};
    #[allow(unused_imports)]
    use crate::look::{LookDocExt as _, LookDocV2Ext as _};
    use crate::Db;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut b = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        b.extend_from_slice(&13u32.to_be_bytes());
        b.extend_from_slice(b"IHDR");
        b.extend_from_slice(&width.to_be_bytes());
        b.extend_from_slice(&height.to_be_bytes());
        b.extend_from_slice(&[8, 2, 0, 0, 0]);
        b.extend_from_slice(&[0; 4]); // CRC, which the sniffer never reads
        b
    }

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
