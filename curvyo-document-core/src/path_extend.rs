//! The three commands that grow one open path: extend it, connect it to another, close it
//! (`specs/0034-pen-path-extension`). Each takes one resolved [`PathGrowth`] and runs one private
//! funnel that resolves and checks everything before its first write, writes the survivor's
//! anchor list, and ends in exactly one commit labelled `extend_path`, `connect_paths` or
//! `close_path` (ADR 0002 §9). The survivor's id, style, place in the stacking order and the ids
//! of its existing nodes are never written.

use std::collections::HashSet;

use crate::document::{Document, OBJECTS_TREE};
use crate::path_codec::{
    KEY_HANDLE_IN, KEY_HANDLE_OUT, KEY_POINT, anchor_index, anchor_map_at, insert_anchor_at,
    read_vec2, write_closed, write_kind, write_point, write_vec2,
};
use crate::path_model::{AnchorId, AnchorSnapshot, NewAnchor, NodeId, PathEditError, PathSnapshot};
use crate::path_reverse::reversed_anchors;

/// One end of an open path's anchor list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathEnd {
    /// The first anchor; growth here is placed before the existing anchors.
    First,
    /// The last anchor; growth here is placed after the existing anchors.
    Last,
}

/// What one command does to one path, resolved by the caller (ADR 0002: commands carry resolved
/// geometry, never geometric intent).
#[derive(Debug, Clone, PartialEq)]
pub struct PathGrowth {
    /// The surviving object: it keeps its id, style, place and the ids of its existing anchors.
    pub path: NodeId,
    /// The end of `path` where `added` and `absorb` attach.
    pub end: PathEnd,
    /// New anchors in stored order. At the first end they are inserted before every existing
    /// anchor, so the caller passes them already in the order they are to be stored (the node
    /// drawn last first).
    pub added: Vec<NewAnchor>,
    /// For a connect: the other path and the end of it that meets `path`. Its anchors follow
    /// `added` (at the first end: precede it), reversed when both joined ends are the same kind
    /// of end, so the two ends are adjacent. The other object is removed.
    pub absorb: Option<(NodeId, PathEnd)>,
    /// Resolved values of existing anchors of `path` or of the absorbed path that change (a
    /// merged junction, a closing node with its join applied). Only the fields that differ are
    /// written.
    pub replace: Vec<AnchorSnapshot>,
    /// Existing anchors of `path` or of the absorbed path that are removed (the end a merge
    /// replaces).
    pub drop: Vec<AnchorId>,
}

/// A growth that passed every check: the survivor's snapshot, the absorbed path's final anchors
/// in the order they are inserted, and whether the result closes.
struct Plan {
    growth: PathGrowth,
    survivor: PathSnapshot,
    absorbed: Option<(NodeId, Vec<NewAnchor>)>,
    close: bool,
}

impl Document {
    /// Adds `growth.added` at `growth.end` of an open ordinary path as one commit labelled
    /// `extend_path` (`0034` criteria 4 and 5). Refuses `absorb`.
    ///
    /// # Errors
    /// [`PathEditError::NoSuchPath`] / [`PathEditError::NoSuchAnchor`] if a path or an anchor
    /// named is gone; [`PathEditError::NotExtendable`] if the path is closed or compound or the
    /// growth absorbs another; [`PathEditError::DuplicateAnchorId`] if an added id is used twice.
    pub fn extend_path(&self, growth: &PathGrowth) -> Result<(), PathEditError> {
        if growth.absorb.is_some() {
            return Err(PathEditError::NotExtendable);
        }
        let plan = self.plan_growth(growth, false)?;
        self.apply_growth(&plan);
        self.commit_with_label("extend_path");
        Ok(())
    }

    /// Grows `growth.path` at `growth.end`, then absorbs the other path at its joined end and
    /// removes it, as one commit labelled `connect_paths` (`0034` criteria 8 to 11). `absorb` may
    /// be left out: a new path that ends on another one becomes that other one.
    ///
    /// # Errors
    /// As [`Document::extend_path`]; [`PathEditError::NotExtendable`] also if the absorbed path
    /// is the survivor itself, closed or compound.
    pub fn connect_paths(&self, growth: &PathGrowth) -> Result<(), PathEditError> {
        let plan = self.plan_growth(growth, false)?;
        self.apply_growth(&plan);
        self.commit_with_label("connect_paths");
        Ok(())
    }

    /// Grows each path (with its `added`, if any) and closes it, all in one commit labelled
    /// `close_path` (`0034` criteria 14 and 19). Every growth is checked before the first write,
    /// so a refusal changes nothing.
    ///
    /// # Errors
    /// As [`Document::extend_path`]; [`PathEditError::TooFewToClose`] if a result would have
    /// fewer than three anchors.
    pub fn close_paths(&self, growths: &[PathGrowth]) -> Result<(), PathEditError> {
        let plans = growths
            .iter()
            .map(|growth| self.plan_growth(growth, true))
            .collect::<Result<Vec<_>, _>>()?;
        for plan in &plans {
            self.apply_growth(plan);
        }
        self.commit_with_label("close_path");
        Ok(())
    }

    /// Resolves and checks one growth without writing anything.
    fn plan_growth(&self, growth: &PathGrowth, close: bool) -> Result<Plan, PathEditError> {
        let survivor = self.path(growth.path).ok_or(PathEditError::NoSuchPath)?;
        if survivor.is_compound() || survivor.closed {
            return Err(PathEditError::NotExtendable);
        }
        let absorbed_snapshot = match growth.absorb {
            Some((other, _)) => {
                if other == growth.path {
                    return Err(PathEditError::NotExtendable);
                }
                let snapshot = self.path(other).ok_or(PathEditError::NoSuchPath)?;
                if snapshot.is_compound() || snapshot.closed {
                    return Err(PathEditError::NotExtendable);
                }
                Some((other, snapshot))
            }
            None => None,
        };
        check_ids(
            growth,
            &survivor,
            absorbed_snapshot.as_ref().map(|(_, s)| s),
        )?;
        let absorbed = absorbed_snapshot.map(|(other, snapshot)| {
            let end = growth.absorb.map_or(PathEnd::First, |(_, end)| end);
            let mut anchors: Vec<NewAnchor> = snapshot
                .anchors
                .iter()
                .filter(|anchor| !growth.drop.contains(&anchor.id))
                .map(|anchor| resolved(anchor, &growth.replace))
                .collect();
            if end == growth.end {
                anchors = reversed_anchors(&anchors);
            }
            (other, anchors)
        });
        let kept = survivor
            .anchors
            .iter()
            .filter(|anchor| !growth.drop.contains(&anchor.id))
            .count();
        let total = kept + growth.added.len() + absorbed.as_ref().map_or(0, |(_, a)| a.len());
        if close && total < 3 {
            return Err(PathEditError::TooFewToClose);
        }
        Ok(Plan {
            growth: growth.clone(),
            survivor,
            absorbed,
            close,
        })
    }

    /// Writes a checked plan: no commit.
    fn apply_growth(&self, plan: &Plan) {
        let growth = &plan.growth;
        // invariant: `plan_growth` resolved this path moments ago.
        #[allow(clippy::unwrap_used)]
        let (meta, anchors) = self.path_parts(growth.path).unwrap();
        for new in &growth.replace {
            if let Some(old) = plan.survivor.anchors.iter().find(|a| a.id == new.id)
                && let Ok(index) = anchor_index(&anchors, new.id)
            {
                write_changed(&anchor_map_at(&anchors, index), old, new);
            }
        }
        let mut doomed: Vec<usize> = growth
            .drop
            .iter()
            .filter_map(|&id| anchor_index(&anchors, id).ok())
            .collect();
        doomed.sort_unstable();
        for index in doomed.into_iter().rev() {
            // invariant: the index was just found in this same list.
            #[allow(clippy::unwrap_used)]
            anchors.delete(index, 1).unwrap();
        }
        let absorbed: &[NewAnchor] = plan.absorbed.as_ref().map_or(&[], |(_, list)| list);
        match growth.end {
            PathEnd::Last => {
                let at = anchors.len();
                for (offset, anchor) in growth.added.iter().chain(absorbed).enumerate() {
                    insert_anchor_at(&anchors, at + offset, anchor);
                }
            }
            PathEnd::First => {
                for (offset, anchor) in absorbed.iter().chain(&growth.added).enumerate() {
                    insert_anchor_at(&anchors, offset, anchor);
                }
            }
        }
        if let Some((other, _)) = &plan.absorbed {
            let tree = self.loro().get_tree(OBJECTS_TREE);
            // invariant: `plan_growth` resolved this object moments ago.
            #[allow(clippy::unwrap_used)]
            tree.delete(crate::paths::tree_id_of(*other)).unwrap();
        }
        if plan.close {
            write_closed(&meta, true);
        }
    }
}

/// `anchor` with the replacement for its id applied, if there is one.
fn resolved(anchor: &AnchorSnapshot, replace: &[AnchorSnapshot]) -> NewAnchor {
    replace
        .iter()
        .find(|new| new.id == anchor.id)
        .copied()
        .unwrap_or(*anchor)
}

/// Writes the fields of `new` that differ from `old`: a redundant write would win against a
/// collaborator's concurrent edit of the same register.
fn write_changed(map: &loro::LoroMap, old: &AnchorSnapshot, new: &AnchorSnapshot) {
    if old.point != new.point {
        write_point(map, KEY_POINT, new.point);
    }
    if old.handle_in != new.handle_in || read_vec2(map, KEY_HANDLE_IN) != new.handle_in {
        write_vec2(map, KEY_HANDLE_IN, new.handle_in);
    }
    if old.handle_out != new.handle_out || read_vec2(map, KEY_HANDLE_OUT) != new.handle_out {
        write_vec2(map, KEY_HANDLE_OUT, new.handle_out);
    }
    if old.kind != new.kind {
        write_kind(map, new.kind);
    }
}

/// The ids of a growth: added ids are unique and belong to no anchor of the two paths, and every
/// replaced or dropped id is an anchor of the survivor or of the absorbed path.
fn check_ids(
    growth: &PathGrowth,
    survivor: &PathSnapshot,
    absorbed: Option<&PathSnapshot>,
) -> Result<(), PathEditError> {
    let existing: HashSet<AnchorId> = survivor
        .anchors
        .iter()
        .chain(absorbed.iter().flat_map(|path| path.anchors.iter()))
        .map(|anchor| anchor.id)
        .collect();
    let mut seen = HashSet::new();
    for anchor in &growth.added {
        if existing.contains(&anchor.id) || !seen.insert(anchor.id) {
            return Err(PathEditError::DuplicateAnchorId);
        }
    }
    let named = growth
        .replace
        .iter()
        .map(|anchor| anchor.id)
        .chain(growth.drop.iter().copied());
    for id in named {
        if !existing.contains(&id) {
            return Err(PathEditError::NoSuchAnchor);
        }
    }
    Ok(())
}
