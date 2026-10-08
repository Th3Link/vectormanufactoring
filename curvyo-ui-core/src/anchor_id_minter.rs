//! Minting fresh [`AnchorId`]s for one session.
//!
//! `specs/0002-path-node-editing/adrs.md`: "`AnchorId` is minted by the
//! creating peer and is globally unique... passed into
//! `curvyo-document-core`, never minted there." Both tools in this crate
//! create anchors — [`crate::PenTool`] placing nodes, [`crate::NodeTool`]
//! inserting one on a double-clicked segment — so they share one minter
//! per session rather than each keeping an independent counter, which
//! would risk two tools minting the same `(peer, counter)` pair.

use curvyo_document_core::{AnchorId, StopId};

/// Mints [`AnchorId`]s for one open session on one peer.
///
/// The caller (`curvyo-editor-wasm`) owns exactly one of these per open
/// document and hands a `&mut` reference into whichever tool is currently
/// handling input.
#[derive(Debug)]
pub struct AnchorIdMinter {
    peer: u64,
    next: u64,
}

impl AnchorIdMinter {
    /// Starts minting for `peer` — the same Loro peer id the session's
    /// [`curvyo_document_core::Document`] was opened with
    /// (`specs/0001-project-file-foundation/adrs.md`, amended 2026-10-03: a
    /// fresh peer id per open session).
    #[must_use]
    pub const fn new(peer: u64) -> Self {
        Self { peer, next: 0 }
    }

    /// Mints the next fresh [`AnchorId`] for this session.
    pub fn mint(&mut self) -> AnchorId {
        let id = AnchorId::new(self.peer, self.next);
        self.next += 1;
        id
    }

    /// Mints the next fresh [`StopId`] for this session, from the same counter
    /// as [`AnchorIdMinter::mint`] (a stop id only has to be unique within its
    /// object's stop list, so sharing the counter costs nothing and the two
    /// kinds can never collide).
    pub fn mint_stop(&mut self) -> StopId {
        let id = StopId::new(self.peer, self.next);
        self.next += 1;
        id
    }

    /// The [`AnchorId`] the *next* [`AnchorIdMinter::mint`] call would
    /// return, without consuming it. For a live preview (e.g.
    /// [`crate::PenTool::pending_anchor`]) that needs to show the id a
    /// not-yet-committed node would actually get: previewing must never
    /// advance this counter itself, or it would desync from what the
    /// gesture might still turn into (Escape discards it, a press turning
    /// out to be a close-path gesture mints nothing) — the same anchor id
    /// could then be minted twice, once by a stale preview and once for
    /// real.
    #[must_use]
    pub const fn peek(&self) -> AnchorId {
        AnchorId::new(self.peer, self.next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successive_mints_are_distinct() {
        let mut minter = AnchorIdMinter::new(1);
        let a = minter.mint();
        let b = minter.mint();
        assert_ne!(a, b);
    }

    #[test]
    fn stop_ids_are_distinct_from_each_other_and_share_the_counter() {
        let mut minter = AnchorIdMinter::new(1);
        let a = minter.mint_stop();
        let anchor = minter.mint();
        let b = minter.mint_stop();
        assert_ne!(a, b);
        assert_eq!(anchor, AnchorId::new(1, 1), "one counter for both kinds");
        assert_eq!(b, StopId::new(1, 2));
    }

    #[test]
    fn two_minters_on_different_peers_never_collide() {
        let mut a = AnchorIdMinter::new(1);
        let mut b = AnchorIdMinter::new(2);
        assert_ne!(a.mint(), b.mint());
    }

    #[test]
    fn peek_reports_what_the_next_mint_will_return_without_consuming_it() {
        let mut minter = AnchorIdMinter::new(1);
        let peeked = minter.peek();
        assert_eq!(peeked, minter.peek(), "peeking twice reports the same id");
        assert_eq!(
            peeked,
            minter.mint(),
            "the next real mint must be exactly what was peeked"
        );
        assert_ne!(
            peeked,
            minter.peek(),
            "peek moves on once something has actually been minted"
        );
    }
}
