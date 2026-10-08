//! The Loro keys of a rectangle's corner radii
//! (`specs/0013-rectangle-corner-radii/adrs.md`, decisions 1, 2, 4 and 10): four flat
//! last-writer-wins registers, one per corner, each a millimetre `f64` stored
//! raw, and the read of the legacy single `corner_radius` key.
//!
//! ```text
//! rect: corner_radius_tl, corner_radius_tr,
//!       corner_radius_br, corner_radius_bl   mm, stored raw, one LWW register each
//!       corner_radius                        legacy (format_version <= 5): read as
//!                                            the radius of every corner that has no
//!                                            key of its own, never written again
//! ```

use loro::LoroMap;

use crate::corner_radii::{Corner, CornerRadii};
use crate::path_codec;
use crate::units::Length;

/// The single radius of every rectangle written before `format_version` 6.
/// Read as the fallback of a corner without a key of its own; never written.
pub(crate) const KEY_LEGACY_CORNER_RADIUS: &str = "corner_radius";
pub(crate) const KEY_CORNER_RADIUS_TL: &str = "corner_radius_tl";
pub(crate) const KEY_CORNER_RADIUS_TR: &str = "corner_radius_tr";
pub(crate) const KEY_CORNER_RADIUS_BR: &str = "corner_radius_br";
pub(crate) const KEY_CORNER_RADIUS_BL: &str = "corner_radius_bl";

const fn key_of(corner: Corner) -> &'static str {
    match corner {
        Corner::Tl => KEY_CORNER_RADIUS_TL,
        Corner::Tr => KEY_CORNER_RADIUS_TR,
        Corner::Br => KEY_CORNER_RADIUS_BR,
        Corner::Bl => KEY_CORNER_RADIUS_BL,
    }
}

fn read_mm(meta: &LoroMap, key: &str) -> Option<f64> {
    path_codec::as_f64(&meta.get(key)?.get_deep_value())
}

/// One corner's stored radius: its own key if present, else the legacy
/// `corner_radius`. `None` when the value that applies is absent or not a
/// number. A mistyped own key is not masked by the legacy key.
fn read_corner(meta: &LoroMap, corner: Corner) -> Option<Length> {
    let own = key_of(corner);
    let key = if meta.get(own).is_some() {
        own
    } else {
        KEY_LEGACY_CORNER_RADIUS
    };
    read_mm(meta, key).map(Length::from_mm)
}

/// The four stored radii of a rectangle node, or `None` if any corner has no
/// usable value.
pub(crate) fn read_corner_radii(meta: &LoroMap) -> Option<CornerRadii> {
    Some(CornerRadii {
        tl: read_corner(meta, Corner::Tl)?,
        tr: read_corner(meta, Corner::Tr)?,
        br: read_corner(meta, Corner::Br)?,
        bl: read_corner(meta, Corner::Bl)?,
    })
}

/// Whether every corner of `meta` has a finite radius of at least 0
/// (`OpenError::Damaged` otherwise). A sum above a side is valid: it is
/// shrunk on evaluation.
pub(crate) fn corner_radii_are_valid(meta: &LoroMap) -> bool {
    read_corner_radii(meta).is_some_and(|radii| {
        Corner::ALL.into_iter().all(|corner| {
            let mm = radii.get(corner).as_mm();
            mm.is_finite() && mm >= 0.0
        })
    })
}

/// Writes `radius` to `corner`'s register unless the corner already reads that
/// value (own key or legacy fallback): an unchanged register is never
/// rewritten, so a rewrite cannot beat a concurrent edit (ADR 0009 §3).
/// Returns whether it wrote.
pub(crate) fn write_corner_radius_if_changed(
    meta: &LoroMap,
    corner: Corner,
    radius: Length,
) -> bool {
    if read_corner(meta, corner) == Some(radius) {
        return false;
    }
    // invariant: inserting a number under a known key on an attached meta map
    // cannot fail.
    #[allow(clippy::unwrap_used)]
    meta.insert(key_of(corner), radius.as_mm()).unwrap();
    true
}

/// Writes each corner of `radii` that differs from what the node reads
/// (see [`write_corner_radius_if_changed`]). Returns whether any register was
/// written.
pub(crate) fn write_corner_radii_if_changed(meta: &LoroMap, radii: CornerRadii) -> bool {
    let mut written = false;
    for corner in Corner::ALL {
        // Not `any`: every corner must be visited, whatever the earlier ones did.
        written |= write_corner_radius_if_changed(meta, corner, radii.get(corner));
    }
    written
}

#[cfg(test)]
mod tests {
    use super::*;
    use loro::LoroDoc;

    fn meta() -> (LoroDoc, LoroMap) {
        let doc = LoroDoc::new();
        let map = doc.get_map("m");
        (doc, map)
    }

    fn radii(tl: f64, tr: f64, br: f64, bl: f64) -> CornerRadii {
        CornerRadii {
            tl: Length::from_mm(tl),
            tr: Length::from_mm(tr),
            br: Length::from_mm(br),
            bl: Length::from_mm(bl),
        }
    }

    #[test]
    fn four_own_keys_read_back_exactly() {
        let (_doc, meta) = meta();
        assert!(write_corner_radii_if_changed(
            &meta,
            radii(1.0, 2.0, 3.0, 4.0)
        ));
        assert_eq!(read_corner_radii(&meta), Some(radii(1.0, 2.0, 3.0, 4.0)));
        assert!(meta.get(KEY_LEGACY_CORNER_RADIUS).is_none());
    }

    #[test]
    fn the_legacy_key_alone_reads_as_four_equal_corners() {
        let (_doc, meta) = meta();
        meta.insert(KEY_LEGACY_CORNER_RADIUS, 2.5).unwrap();
        assert_eq!(read_corner_radii(&meta), Some(radii(2.5, 2.5, 2.5, 2.5)));
    }

    #[test]
    fn a_corner_reads_its_own_key_before_the_legacy_one() {
        let (_doc, meta) = meta();
        meta.insert(KEY_LEGACY_CORNER_RADIUS, 2.5).unwrap();
        meta.insert(KEY_CORNER_RADIUS_BR, 7.0).unwrap();
        assert_eq!(read_corner_radii(&meta), Some(radii(2.5, 2.5, 7.0, 2.5)));
    }

    #[test]
    fn a_corner_with_no_key_and_no_legacy_key_is_unreadable() {
        let (_doc, meta) = meta();
        meta.insert(KEY_CORNER_RADIUS_TL, 1.0).unwrap();
        meta.insert(KEY_CORNER_RADIUS_TR, 1.0).unwrap();
        meta.insert(KEY_CORNER_RADIUS_BR, 1.0).unwrap();
        assert_eq!(read_corner_radii(&meta), None);
        assert!(!corner_radii_are_valid(&meta));
    }

    #[test]
    fn a_mistyped_own_key_is_not_masked_by_the_legacy_key() {
        let (_doc, meta) = meta();
        meta.insert(KEY_LEGACY_CORNER_RADIUS, 2.5).unwrap();
        meta.insert(KEY_CORNER_RADIUS_TL, "big").unwrap();
        assert_eq!(read_corner_radii(&meta), None);
    }

    #[test]
    fn a_mistyped_legacy_key_matters_only_where_a_corner_falls_back_to_it() {
        let (_doc, meta) = meta();
        meta.insert(KEY_LEGACY_CORNER_RADIUS, "x").unwrap();
        assert!(!corner_radii_are_valid(&meta), "every corner falls back");
        for (key, value) in [
            (KEY_CORNER_RADIUS_TL, 1.0),
            (KEY_CORNER_RADIUS_TR, 2.0),
            (KEY_CORNER_RADIUS_BR, 3.0),
        ] {
            meta.insert(key, value).unwrap();
        }
        assert!(!corner_radii_are_valid(&meta), "BL still falls back");
        meta.insert(KEY_CORNER_RADIUS_BL, 4.0).unwrap();
        assert!(
            corner_radii_are_valid(&meta),
            "no corner reads the legacy key"
        );
    }

    #[test]
    fn negative_and_non_finite_radii_are_invalid() {
        for bad in [-0.5, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let (_doc, meta) = meta();
            write_corner_radii_if_changed(&meta, radii(1.0, 1.0, bad, 1.0));
            assert!(!corner_radii_are_valid(&meta), "{bad}");
        }
        let (_doc, meta) = meta();
        meta.insert(KEY_LEGACY_CORNER_RADIUS, -1.0).unwrap();
        assert!(!corner_radii_are_valid(&meta));
    }

    #[test]
    fn a_stored_sum_above_a_side_is_not_invalid() {
        let (_doc, meta) = meta();
        write_corner_radii_if_changed(&meta, radii(500.0, 500.0, 500.0, 500.0));
        assert!(corner_radii_are_valid(&meta));
    }

    #[test]
    fn an_unchanged_register_is_not_rewritten() {
        let (doc, meta) = meta();
        assert!(write_corner_radii_if_changed(
            &meta,
            radii(1.0, 2.0, 3.0, 4.0)
        ));
        doc.commit();
        let before = doc.oplog_vv();
        assert!(!write_corner_radii_if_changed(
            &meta,
            radii(1.0, 2.0, 3.0, 4.0)
        ));
        doc.commit();
        assert_eq!(doc.oplog_vv(), before);
    }

    #[test]
    fn only_the_changed_registers_are_written() {
        let (doc, meta) = meta();
        write_corner_radii_if_changed(&meta, radii(1.0, 2.0, 3.0, 4.0));
        doc.commit();
        let before = doc.oplog_vv();
        assert!(write_corner_radii_if_changed(
            &meta,
            radii(1.0, 9.0, 3.0, 8.0)
        ));
        doc.commit();
        let after = doc.oplog_vv();
        let peer = doc.peer_id();
        let ops = |vv: &loro::VersionVector| vv.get(&peer).copied().unwrap_or(0);
        assert_eq!(ops(&after) - ops(&before), 2, "TR and BL only");
        assert_eq!(read_corner_radii(&meta), Some(radii(1.0, 9.0, 3.0, 8.0)));
    }

    #[test]
    fn a_value_equal_to_the_legacy_fallback_writes_nothing() {
        let (_doc, meta) = meta();
        meta.insert(KEY_LEGACY_CORNER_RADIUS, 2.0).unwrap();
        assert!(!write_corner_radii_if_changed(
            &meta,
            radii(2.0, 2.0, 2.0, 2.0)
        ));
        assert!(meta.get(KEY_CORNER_RADIUS_TL).is_none());
    }
}
