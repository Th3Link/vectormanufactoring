//! Impure capabilities the desktop client needs, one module each
//! (ADR 0011 §2). This slice only needs the filesystem module; the other
//! modules (OS CSPRNG, clock, credential store, HTTP, git wire, relay
//! socket) belong to later stories and get no code here
//! (`specs/0001-project-file-foundation/adrs.md`).

// `CLAUDE.md` §5 allows unwrap/expect in tests; only production code is held
// to the stricter rule.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod filesystem;

pub use filesystem::{FsError, read_to_vec, write_atomic};
