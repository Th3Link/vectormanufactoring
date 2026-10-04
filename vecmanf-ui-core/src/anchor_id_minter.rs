//! Minting fresh [`AnchorId`]s for one session.
//!
//! `specs/0002-path-node-editing/adrs.md`: "`AnchorId` is minted by the
//! creating peer and is globally unique... passed into
//! `vecmanf-document-core`, never minted there." Both tools in this crate
//! create anchors — [`crate::PenTool`] placing nodes, [`crate::NodeTool`]
//! inserting one on a double-clicked segment — so they share one minter
//! per session rather than each keeping an independent counter, which
//! would risk two tools minting the same `(peer, counter)` pair.

use vecmanf_document_core::AnchorId;

/// Mints [`AnchorId`]s for one open session on one peer.
///
/// The caller (`vecmanf-editor-wasm`) owns exactly one of these per open
/// document and hands a `&mut` reference into whichever tool is currently
/// handling input.
#[derive(Debug)]
pub struct AnchorIdMinter {
    peer: u64,
    next: u64,
}

impl AnchorIdMinter {
    /// Starts minting for `peer` — the same Loro peer id the session's
    /// [`vecmanf_document_core::Document`] was opened with
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
    fn two_minters_on_different_peers_never_collide() {
        let mut a = AnchorIdMinter::new(1);
        let mut b = AnchorIdMinter::new(2);
        assert_ne!(a.mint(), b.mint());
    }
}
