//! The one place that still knows gradients: version-7 files written by a build
//! of `0007` PR 4 may hold `fill_kind` and `fill_stops`
//! (`specs/0017-style-panel-rework/adrs.md`, decision 1).
//!
//! **Read:** a fill paints only if `fill_enabled` and `fill_kind` is absent or
//! `"solid"`; any other kind reads as a fill that is off. `fill_stops` is never
//! read. **Write:** whenever a fill key is written, both legacy keys are deleted
//! in the same commit, so the new fill replaces the old one. Opening and saving
//! without a fill edit leaves the bytes alone. Delete this module once no such
//! file is expected (`docs/technical-debt.md`).

use loro::{LoroMap, LoroValue};

use crate::style_codec::KEY_FILL_ENABLED;

pub(crate) const KEY_FILL_KIND: &str = "fill_kind";
pub(crate) const KEY_FILL_STOPS: &str = "fill_stops";

const KIND_SOLID: &str = "solid";
const KIND_LINEAR: &str = "linear";
const KIND_RADIAL: &str = "radial";

fn kind_of(meta: &LoroMap) -> Option<String> {
    match meta.get(KEY_FILL_KIND)?.get_deep_value() {
        LoroValue::String(kind) => Some(kind.to_string()),
        _ => None,
    }
}

/// Whether the stored fill is a solid one (the only kind that exists now): the
/// kind key is absent or says so. Anything else reads as no fill.
pub(crate) fn reads_as_solid(meta: &LoroMap) -> bool {
    kind_of(meta).is_none_or(|kind| kind == KIND_SOLID)
}

/// Whether `value` is a `fill_kind` an open file may hold: a file with another
/// string is damaged, as it always was.
pub(crate) fn is_valid_kind(value: &LoroValue) -> bool {
    matches!(
        value,
        LoroValue::String(s) if [KIND_SOLID, KIND_LINEAR, KIND_RADIAL].contains(&s.as_str())
    )
}

/// Drops the legacy keys of an object whose fill is about to be written. A
/// gradient fill that read as off stays off until the write says otherwise:
/// its `fill_enabled` is set to `false` first, and the write that follows
/// (Paint Solid) overrides it in the same commit.
pub(crate) fn forget(meta: &LoroMap) {
    let had_gradient = !reads_as_solid(meta);
    for key in [KEY_FILL_KIND, KEY_FILL_STOPS] {
        if meta.get(key).is_some() {
            // invariant: deleting a present key on an attached map cannot fail.
            #[allow(clippy::unwrap_used)]
            meta.delete(key).unwrap();
        }
    }
    if had_gradient {
        // invariant: inserting a plain value under a known key cannot fail.
        #[allow(clippy::unwrap_used)]
        meta.insert(KEY_FILL_ENABLED, false).unwrap();
    }
}
