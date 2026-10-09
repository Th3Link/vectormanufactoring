//! The document model's root: a Loro-backed replica carrying exactly the
//! fields this slice needs — `format_version` and `size` — as per-field
//! last-writer-wins registers (ADR 0009 §3; ADR 0002 §1, §9; ADR 0004 §2,
//! §3; `specs/0001-project-file-foundation/adrs.md`, "a minimal document root
//! record, and no more").

use loro::{CommitOptions, ExportMode, LoroDoc, LoroMap, LoroValue};
use serde::Serialize;

use crate::display_unit::DisplayUnit;
use crate::document_size::{MAX_DOCUMENT_MM, MIN_DOCUMENT_MM};
use crate::error::{OpenError, SaveError};
use crate::units::{DocumentSize, Length};

/// The container `format_version` this build writes and the newest it
/// accepts on read (ADR 0004 §9).
///
/// Bumped to 3 in `primitive-shapes`: a version-2 reader would refuse a
/// primitive node as `OpenError::Damaged` (it has no `anchors`), which is
/// the wrong message for a file this build can actually read — ADR 0004
/// §9 wants "newer version" instead (`specs/0003-primitive-shapes/adrs.md`,
/// "`format_version` goes to 3"). Migration from version 2 is empty by
/// construction: absent `shape` means path, so every version-2 path node
/// opens unchanged.
///
/// Bumped to 4 in `path-merge-split-and-node-types` (`specs/0006-path-
/// merge-split-and-node-types/adrs.md`, architect's 2026-10-05
/// resolution): this slice merges before `object-transform` has a
/// branch at all, so it takes the next free version outright —
/// `object-transform` is sequenced after it and takes 5,
/// `stroke-and-fill-styling` takes 6. The standing rule going forward:
/// a version number named in an `adrs.md` is provisional until its PR
/// actually merges; whichever one merges first takes `main`'s
/// `CURRENT_FORMAT_VERSION + 1`, and a later PR rebases and renumbers
/// itself, its fixtures and its own notes. A version-3 reader maps any
/// unknown `kind` string to `Corner` (slice 2's lenient read), so it
/// would open a version-4 file with `Asymmetric` anchors and silently
/// turn them into corners — the silent partial read ADR 0004 §9
/// forbids. Migration from version 3 is the `"smooth"` → `Symmetric`
/// read alias in `path_codec::read_kind` and nothing else: no anchor's
/// stored shape changes, only the kind tag's legacy spelling is now
/// also understood.
///
/// Bumped to 5 in `object-transform`, exactly as this comment's own
/// previous paragraph already anticipated: it is the first slice to
/// merge after this one, so it takes `CURRENT_FORMAT_VERSION + 1`
/// outright, no renumbering needed. `document.json` and the Loro root
/// gain a `rotation` key on each object (`specs/0005-object-transform/
/// adrs.md`). A version-4 reader has no `rotation` key at all, so it
/// would open a rotated object and draw it unrotated — the silent
/// partial read ADR 0004 §9 forbids. Migration from version 4 is empty
/// by construction: absent `rotation` reads as `0`
/// (`path_codec::read_rotation`), so every version-4 object opens with
/// zero rotation, unchanged.
///
/// Bumped to 6 in `rectangle-corner-radii` (`specs/0013-rectangle-corner-radii/
/// adrs.md`, decisions 2 and 3): a rectangle stores four corner radius
/// registers (`corner_radius_tl`, `_tr`, `_br`, `_bl`) and the single
/// `corner_radius` key is no longer written. A version-5 reader would refuse
/// every rectangle this build writes as `OpenError::Damaged` (no
/// `corner_radius`), the wrong message; with differing radii it must not open
/// the file at all. The bump makes it say "saved by a newer version", for
/// every file this build saves. Migration from version 5 is empty: each corner
/// reads its own key, else the legacy `corner_radius`, else the file is
/// damaged, and an old file is not rewritten on open. The number is
/// provisional by the rule above: whichever PR merges takes `main`'s current
/// number plus one.
///
/// Bumped to 7 in `stroke-and-fill-styling` (`specs/0007-stroke-and-fill-
/// styling/adrs.md`, "`format_version`" and the 2026-10-07 readiness check):
/// every object node may carry the style keys of `crate::style_codec`
/// (`stroke_enabled`, `stroke_opacity`, `stroke_dash`, `stroke_join`,
/// `stroke_cap`, `fill_enabled`, `fill`, `fill_opacity`; a build of that
/// version may also have written the two keys `crate::legacy_fill` reads past),
/// and `document.json` writes one `style` object per object. A version-6
/// reader tolerates unknown keys, so it would open a version-7 file and draw
/// every dash and fill as a thin black outline, the silent partial read ADR 0004 §9 forbids; with the bump it
/// says "saved by a newer version". Migration from version 6 is empty by
/// construction: every new key is absent in an older file and an absent key
/// reads as the frozen default, which is what older builds drew (0.25 mm
/// solid black stroke, no fill), and an old file is not rewritten on open.
/// The stroke `stroke_width` and `stroke` keys keep their meaning. This is
/// the complete format of the slice: later parts of the story add no key
/// and need no further bump. The number is provisional by the rule above.
///
/// The fill keys `fill_kind` and `fill_stops` of a version-7 file are not part
/// of the format any more (`specs/0017-style-panel-rework/adrs.md`, decision 1):
/// `crate::legacy_fill` reads past them and drops them on the next fill write,
/// which needs no bump because nothing is misread in either direction.
///
/// Bumped to 8 in `boolean-operations` (`specs/0016-boolean-operations/
/// adrs.md`, "compound path"): a path may hold several outlines, the first in
/// the ordinary `closed`/`anchors` keys and the others in one new optional
/// movable list, `extra_subpaths` (`crate::subpath_codec`). A build that stops
/// at version 7 would read such a path as its first outline alone and drop
/// the rest without a word, the silent partial read ADR 0004 §9 forbids, so
/// it must refuse the file as too new. Migration from version 7 is none: the
/// key is simply absent, and a file from an earlier build opens unchanged and
/// is not rewritten. Golden: `compound_v8.curvyo`.
pub const CURRENT_FORMAT_VERSION: u32 = 8;

pub(crate) const ROOT_MAP: &str = "root";
const KEY_FORMAT_VERSION: &str = "format_version";
pub(crate) const KEY_WIDTH_MM: &str = "size_width_mm";
pub(crate) const KEY_HEIGHT_MM: &str = "size_height_mm";
/// The root register holding the display unit's symbol. Added without a
/// `format_version` bump: an older build ignores the key, shows mm, and keeps
/// the key when it saves (`specs/0015-document-size-and-rulers/adrs.md`,
/// decision 1).
pub(crate) const KEY_DISPLAY_UNIT: &str = "display_unit";

/// The top-level Loro tree container holding every object — path or
/// primitive alike (ADR 0002 §5: sibling order among tree nodes is
/// z-order; `specs/0003-primitive-shapes/adrs.md`: "primitives and paths share
/// one z-order, so they share one tree. They are not two lists.").
///
/// The Rust constant is named `OBJECTS_TREE` as of `primitive-shapes`,
/// but the on-disk/in-CRDT container key stays the literal `"paths"` it
/// has always been: renaming the stored key would be a container
/// migration for no benefit, since it is an opaque name in a format this
/// crate already writes (`adrs.md`). `curvyo-document-core` is the only
/// module that names this key; everything else goes through
/// [`Document`]'s methods — see [`crate::paths`] and [`crate::shapes`].
pub(crate) const OBJECTS_TREE: &str = "paths";

/// An open Curvyo document.
///
/// Its Loro CRDT backing is an implementation detail and never appears in
/// this type's public API (ADR 0004 §3): callers only ever see
/// [`DocumentSize`] and this crate's own error types.
pub struct Document {
    loro: LoroDoc,
}

impl Document {
    /// Creates a new, empty document at the default page size (A4
    /// portrait, 210 × 297 mm), bound to the given Loro peer id.
    ///
    /// The peer id is a parameter rather than something this crate mints
    /// itself: `LoroDoc::new()` would otherwise draw one from `getrandom`,
    /// and a `*-core` crate reaching an entropy source is exactly what
    /// `CLAUDE.md` §6 forbids ("no filesystem, network, clock, threads or
    /// UI. Callers pass everything in.") — the same reason
    /// `curvyo-library-core` takes a record's UUID as a parameter instead
    /// of generating it (ADR 0011 §6). `curvyo-app` mints a fresh id per
    /// open session and passes it in here
    /// (`specs/0001-project-file-foundation/adrs.md`, amended 2026-10-03).
    ///
    /// # Panics
    /// Does not panic in practice: it only inserts known-valid keys into a
    /// freshly created, attached root map, and sets the peer id before any
    /// operation has been recorded — both of which Loro's API cannot
    /// reject.
    #[must_use]
    pub fn new(peer_id: u64) -> Self {
        let loro = LoroDoc::new();
        // invariant: setting the peer id on a freshly created doc with no
        // recorded operations yet cannot fail.
        #[allow(clippy::unwrap_used)]
        loro.set_peer_id(peer_id).unwrap();
        let root = loro.get_map(ROOT_MAP);
        let size = DocumentSize::default();
        // invariant: inserting known-valid keys into a freshly created,
        // attached root map cannot fail.
        #[allow(clippy::unwrap_used)]
        {
            root.insert(KEY_FORMAT_VERSION, CURRENT_FORMAT_VERSION)
                .unwrap();
            root.insert(KEY_WIDTH_MM, size.width.as_mm()).unwrap();
            root.insert(KEY_HEIGHT_MM, size.height.as_mm()).unwrap();
        }
        Self { loro }
    }

    /// Rebuilds a document from a previously exported Loro snapshot — the
    /// container's `document.loro` member, the sole source of truth on a
    /// normal open (ADR 0004 §1).
    ///
    /// `peer_id` is a parameter for the same reason [`Document::new`]'s is:
    /// `LoroDoc::new()` would otherwise draw one from `getrandom` itself,
    /// which this `*-core` crate must not do (`CLAUDE.md` §6). Reopening a
    /// file is a fresh session like any other, so it gets a fresh id from
    /// the caller the same way a brand-new document does.
    ///
    /// # Errors
    /// Returns [`OpenError::Damaged`] if the bytes are not a valid Loro
    /// snapshot this build can import, or if they import cleanly but the
    /// `paths` tree inside them does not match the shape
    /// [`crate::path_codec`]'s writers always produce — a damaged-but-
    /// still-unzippable file (project-file-foundation's acceptance
    /// criterion 7: "not a crash") rather than a panic the first time some
    /// other method reads that path.
    ///
    /// # Panics
    /// Does not panic in practice: setting the peer id on a freshly
    /// created doc, before `import` records anything, cannot fail.
    pub(crate) fn from_loro_snapshot(peer_id: u64, bytes: &[u8]) -> Result<Self, OpenError> {
        let loro = LoroDoc::new();
        // invariant: setting the peer id on a freshly created doc with no
        // recorded operations yet cannot fail.
        #[allow(clippy::unwrap_used)]
        loro.set_peer_id(peer_id).unwrap();
        loro.import(bytes).map_err(|_| OpenError::Damaged)?;
        if !crate::path_codec::validate_path_tree(&loro, OBJECTS_TREE) {
            return Err(OpenError::Damaged);
        }
        Ok(Self { loro })
    }

    /// The document's size in millimetres.
    ///
    /// Both axes fall back to the A4 default if either stored value is
    /// missing, not a finite number, or outside
    /// [`MIN_DOCUMENT_MM`]`..=`[`MAX_DOCUMENT_MM`]
    /// (`specs/0015-document-size-and-rulers/` criterion 13). The read is
    /// defensive and writes nothing: a file written by a build that stored
    /// no size, or damaged by hand, opens at A4 instead of being refused,
    /// and stays as it is until the maker resizes.
    #[must_use]
    pub fn size(&self) -> DocumentSize {
        let default = DocumentSize::default();
        let root = self.loro.get_map(ROOT_MAP);
        let valid = |value: Option<f64>| {
            value.filter(|mm| (MIN_DOCUMENT_MM..=MAX_DOCUMENT_MM).contains(mm))
        };
        match (
            valid(Self::read_mm(&root, KEY_WIDTH_MM)),
            valid(Self::read_mm(&root, KEY_HEIGHT_MM)),
        ) {
            (Some(width), Some(height)) => {
                DocumentSize::new(Length::from_mm(width), Length::from_mm(height))
            }
            _ => default,
        }
    }

    /// This document's underlying Loro replica, for [`crate::paths`]'s
    /// methods only — never exposed outside this crate (ADR 0004 §3).
    pub(crate) const fn loro(&self) -> &LoroDoc {
        &self.loro
    }

    /// Commits the pending auto-commit transaction with `label` as its
    /// (persisted) commit message, so each editing method in
    /// [`crate::paths`] ends its own transaction instead of letting every
    /// edit since the last explicit commit pile into one (ADR 0002 §9;
    /// `specs/0002-path-node-editing/adrs.md`'s PR review: "a pen session is
    /// one commit" only holds if *every* mutating method commits its own
    /// work, undo-readiness for a future undo/redo feature depends on
    /// one commit per interaction).
    pub(crate) fn commit_with_label(&self, label: &str) {
        self.loro
            .commit_with(CommitOptions::new().commit_msg(label));
    }

    fn read_mm(root: &LoroMap, key: &str) -> Option<f64> {
        match root.get(key)?.get_deep_value() {
            LoroValue::Double(value) => Some(value),
            _ => None,
        }
    }

    /// Exports this document's Loro CRDT snapshot — the authoritative
    /// container member, `document.loro` (ADR 0004 §1).
    ///
    /// # Errors
    /// Returns [`SaveError::Encode`] if the snapshot cannot be encoded.
    pub fn export_loro_snapshot(&self) -> Result<Vec<u8>, SaveError> {
        self.loro
            .export(ExportMode::Snapshot)
            .map_err(|_| SaveError::Encode)
    }

    /// Exports a plain, non-authoritative JSON view of the current state —
    /// the container's `document.json` member (ADR 0004 §1) — read from
    /// this one immutable snapshot of the document rather than live state
    /// (ADR 0009 §4).
    ///
    /// `primitive-shapes` renames this view's array from `paths` to
    /// `objects`, in z-order, each entry tagged with its own `shape`
    /// (`"path"` for a path — `specs/0003-primitive-shapes/adrs.md`,
    /// "`document.json`'s `paths` key → `objects`").
    ///
    /// # Errors
    /// Returns [`SaveError::Encode`] if the view cannot be serialized.
    pub fn export_json(&self) -> Result<Vec<u8>, SaveError> {
        let objects = self
            .object_ids()
            .into_iter()
            .filter_map(|id| self.object(id))
            .map(ObjectJson::from)
            .collect();
        let view = DocumentJsonView {
            format_version: CURRENT_FORMAT_VERSION,
            size: self.size(),
            display_unit: self.display_unit(),
            objects,
        };
        serde_json::to_vec_pretty(&view).map_err(|_| SaveError::Encode)
    }
}

// Deliberately no `impl Default for Document`: there is no safe default
// peer id to mint without a caller-supplied source of randomness, which is
// exactly what `Document::new`'s `peer_id` parameter exists to avoid this
// crate doing itself.

/// The shape of the non-authoritative `document.json` export
/// (ADR 0004 §1).
#[derive(Serialize)]
struct DocumentJsonView {
    format_version: u32,
    size: DocumentSize,
    display_unit: DisplayUnit,
    objects: Vec<ObjectJson>,
}

/// One `document.json` object entry, tagged by its own `shape`
/// (`"path"` for a path) rather than serde's default untagged-enum
/// shape, so a non-Rust reader of this non-authoritative view can tell
/// the five kinds apart without guessing from which fields are present.
#[derive(Serialize)]
#[serde(tag = "shape", rename_all = "lowercase")]
enum ObjectJson {
    Path {
        id: crate::path_model::NodeId,
        closed: bool,
        style: crate::style_model::Style,
        anchors: Vec<crate::path_model::AnchorSnapshot>,
        /// A compound path's further outlines; left out when there are none,
        /// so an ordinary path exports as before.
        #[serde(skip_serializing_if = "Vec::is_empty")]
        extra_subpaths: Vec<crate::path_model::SubpathSnapshot>,
        /// The path's own `rotation` register (`specs/0005-object-
        /// transform/adrs.md`): orientation only, never consulted to
        /// reconstruct geometry — see [`crate::path_model::PathSnapshot::rotation`].
        rotation: crate::units::Angle,
    },
    Rect {
        id: crate::path_model::NodeId,
        bounds: crate::primitive_model::RectBounds,
        corner_radii: crate::corner_radii::CornerRadii,
        style: crate::style_model::Style,
        rotation: crate::units::Angle,
    },
    Ellipse {
        id: crate::path_model::NodeId,
        frame: crate::primitive_model::EllipseFrame,
        style: crate::style_model::Style,
        rotation: crate::units::Angle,
    },
    Polygon {
        id: crate::path_model::NodeId,
        frame: crate::primitive_model::StarFrame,
        point_count: u32,
        style: crate::style_model::Style,
        rotation: crate::units::Angle,
    },
    Star {
        id: crate::path_model::NodeId,
        frame: crate::primitive_model::StarFrame,
        point_count: u32,
        inner_ratio: f64,
        style: crate::style_model::Style,
        rotation: crate::units::Angle,
    },
}

impl From<crate::primitive_model::ObjectSnapshot> for ObjectJson {
    fn from(snapshot: crate::primitive_model::ObjectSnapshot) -> Self {
        use crate::primitive_model::{ObjectSnapshot, Shape};
        match snapshot {
            ObjectSnapshot::Path(path) => Self::Path {
                id: path.id,
                closed: path.closed,
                style: path.style,
                anchors: path.anchors,
                extra_subpaths: path.extra_subpaths,
                rotation: path.rotation,
            },
            ObjectSnapshot::Primitive(primitive) => {
                let id = primitive.id;
                let style = primitive.style;
                let rotation = primitive.rotation;
                match primitive.shape {
                    Shape::Rect {
                        bounds,
                        corner_radii,
                    } => Self::Rect {
                        id,
                        bounds,
                        corner_radii,
                        style,
                        rotation,
                    },
                    Shape::Ellipse { frame } => Self::Ellipse {
                        id,
                        frame,
                        style,
                        rotation,
                    },
                    Shape::Polygon { frame, point_count } => Self::Polygon {
                        id,
                        frame,
                        point_count: point_count.get(),
                        style,
                        rotation,
                    },
                    Shape::Star {
                        frame,
                        point_count,
                        inner_ratio,
                    } => Self::Star {
                        id,
                        frame,
                        point_count: point_count.get(),
                        inner_ratio: inner_ratio.get(),
                        style,
                        rotation,
                    },
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_document_defaults_to_a4_portrait() {
        let document = Document::new(1);
        let size = document.size();
        assert!((size.width.as_mm() - 210.0).abs() < f64::EPSILON);
        assert!((size.height.as_mm() - 297.0).abs() < f64::EPSILON);
    }

    #[test]
    fn new_document_uses_the_given_peer_id() {
        let document = Document::new(0x1234_5678_9abc_def0);
        assert_eq!(document.loro.peer_id(), 0x1234_5678_9abc_def0);
    }

    #[test]
    fn loro_snapshot_round_trips_the_size() {
        let original = Document::new(1);
        let snapshot = original.export_loro_snapshot().expect("export");
        let reopened = Document::from_loro_snapshot(2, &snapshot).expect("import");
        assert_eq!(original.size(), reopened.size());
    }

    #[test]
    fn from_loro_snapshot_uses_the_given_peer_id() {
        let original = Document::new(1);
        let snapshot = original.export_loro_snapshot().expect("export");
        let reopened =
            Document::from_loro_snapshot(0x1234_5678_9abc_def0, &snapshot).expect("import");
        assert_eq!(reopened.loro.peer_id(), 0x1234_5678_9abc_def0);
    }

    #[test]
    fn json_export_matches_the_live_size() {
        let document = Document::new(1);
        let json = document.export_json().expect("export");
        let value: serde_json::Value = serde_json::from_slice(&json).expect("parse");
        assert_eq!(value["format_version"], CURRENT_FORMAT_VERSION);
        assert!((value["size"]["width"].as_f64().unwrap() - 210.0).abs() < f64::EPSILON);
    }
}
