//! The one read of every object out of the document that a frame and the events between two
//! frames share, kept for as long as the document version is the one it was read at
//! (`specs/0044-editing-quick-wins/adrs.md`, decision 5).

use std::cell::RefCell;
use std::sync::Arc;

use curvyo_document_core::{Document, DocumentVersion, ObjectSnapshot};

/// The cached read and whether a drag holds it.
#[derive(Debug, Default)]
struct Held {
    read: Option<(DocumentVersion, Arc<[ObjectSnapshot]>)>,
    /// A Select or Node-tool drag keeps its first read for its whole life, whatever the document
    /// does: it writes nothing until its release, so only a merge could change the document
    /// under it, and that is seen by the first read after the drag (`0031` criterion 17).
    pinned: bool,
}

/// Every object in z-order, read once per document version.
#[derive(Debug, Default)]
pub(super) struct ObjectCache {
    held: RefCell<Held>,
}

impl ObjectCache {
    /// The objects of `document`. With `pin` (a drag is in flight) the read already held is
    /// returned unchanged, so a frame of a drag never reads the document; without it the read is
    /// returned only if the document is still at the version it was made at. Everything else
    /// (`New`, `Open`) builds a new `Session` and with it an empty cache.
    pub(super) fn get(&self, document: &Document, pin: bool) -> Arc<[ObjectSnapshot]> {
        let mut held = self.held.borrow_mut();
        if pin
            && held.pinned
            && let Some((_, objects)) = &held.read
        {
            return Arc::clone(objects);
        }
        held.pinned = pin;
        let version = document.version();
        let objects = match &held.read {
            Some((read_at, objects)) if *read_at == version => Arc::clone(objects),
            _ => {
                let objects = read_objects(document);
                held.read = Some((version, Arc::clone(&objects)));
                objects
            }
        };
        #[cfg(test)]
        assert_eq!(
            format!("{objects:?}"),
            format!("{:?}", read_objects(document)),
            "the cached objects differ from the document: a write did not change its version"
        );
        objects
    }
}

/// Reads every object out of the document, in z-order.
fn read_objects(document: &Document) -> Arc<[ObjectSnapshot]> {
    document
        .object_ids()
        .into_iter()
        .filter_map(|id| document.object(id))
        .collect()
}

#[cfg(test)]
mod tests {
    use curvyo_document_core::{AnchorId, NewAnchor, Point};

    use super::*;

    fn add_path(document: &Document, n: u64) {
        let _ = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, n), Point::new(0.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, n + 1), Point::new(10.0, 0.0)),
            ],
            false,
        );
    }

    #[test]
    fn a_second_read_at_the_same_version_is_the_same_allocation() {
        let document = Document::new(1);
        add_path(&document, 1);
        let cache = ObjectCache::default();
        let first = cache.get(&document, false);
        let second = cache.get(&document, false);
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(first.len(), 1);
    }

    #[test]
    fn a_commit_makes_the_next_read_fresh() {
        let document = Document::new(1);
        add_path(&document, 1);
        let cache = ObjectCache::default();
        let before = cache.get(&document, false);
        add_path(&document, 3);
        let after = cache.get(&document, false);
        assert!(!Arc::ptr_eq(&before, &after));
        assert_eq!((before.len(), after.len()), (1, 2));
    }

    /// The held read of a drag survives a commit and is dropped by the first read after it.
    #[test]
    fn a_pinned_read_survives_a_commit_until_the_drag_ends() {
        let document = Document::new(1);
        add_path(&document, 1);
        let cache = ObjectCache::default();
        let start = cache.get(&document, true);
        let during = cache.get(&document, true);
        assert!(Arc::ptr_eq(&start, &during));
        let ids = document.object_ids();
        let _ = document.translate_objects(&ids, curvyo_document_core::Vec2::new(5.0, 0.0));
        assert!(
            Arc::ptr_eq(&start, &cache.get(&document, true)),
            "a drag keeps its read"
        );
        let after = cache.get(&document, false);
        assert!(!Arc::ptr_eq(&start, &after));
        assert_ne!(
            &start[..],
            &after[..],
            "the read after the drag sees the move"
        );
    }

    #[test]
    fn a_session_can_move_to_another_thread() {
        fn assert_send<T: Send>() {}
        assert_send::<super::super::Session>();
    }
}
