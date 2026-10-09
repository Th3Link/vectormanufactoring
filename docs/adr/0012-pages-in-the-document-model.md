# ADR 0012: Pages in the document model

**Status:** Rejected, 2026-10-05 — the customer dropped multi-page support from
the MVP before this ADR was ever accepted ("nicht wichtig genug für MVP"), so
the decision below never took effect. Kept for its content (the three storage
options and why B/C lose) in case multi-page support is revisited later; a
document is still today's single root map plus one object tree, unchanged.

Extends [ADR 0002](0002-document-model-units-and-svg-round-trip.md) §5. It
supersedes no accepted text. It replaces the shape of the "minimal document
root record" in
[`specs/0001-project-file-foundation/adrs.md`](../../specs/0001-project-file-foundation/adrs.md),
which was a feature-local note, not an ADR.

## Context

Today a document is one root map (`format_version`, `size_width_mm`,
`size_height_mm`) and one Loro tree whose root nodes are the objects. Sibling
order among them is z-order (ADR 0002 §5). The customer asked for several
pages in one project, each with its own size and its own objects, switched
one at a time (`specs/0015-document-size-and-rulers/specification.md`).

Three storage options were considered:

- **A. Pages are the root nodes of the existing object tree** (chosen). Each
  page node holds its size; its objects are its children.
- **B. A movable list of page maps, each holding its own nested object
  tree.** Lost because an object cannot move between two Loro trees: it would
  have to be copied, which mints a new `NodeId` and breaks §5's promise that a
  node's identity lasts for the document's lifetime. Migrating an existing
  file would copy every object into the new tree and change every id. Every
  id lookup would also need to know which tree to search.
- **C. A flat page list plus a `page` register on every object.** Lost
  because page membership becomes a second structure beside the tree.
  Deleting a page means deleting each object that names it, and an object a
  peer adds concurrently survives as an orphan pointing at a missing page.

For the active page there were two options: a shared register in the
document, or per-peer ephemeral state. A shared register is last-writer-wins
(ADR 0009 §3), so one collaborator switching pages would switch everyone.

## Decision

1. **A document has an ordered list of one or more pages. Each page is a root
   node of the object tree.** Its meta map holds `width_mm` and `height_mm`,
   each a per-field last-writer-wins register (ADR 0009 §3). Root sibling
   order is page order. Every object is a descendant of exactly one page, and
   sibling order under a page is that page's z-order. The root map keeps
   `format_version` and loses the size keys.
2. **Each page has its own coordinate space**: millimetres, Y-down, origin at
   the page's top-left corner (ADR 0002 §2, §4). Objects store page-local
   coordinates. If side-by-side pages are ever wanted, a page position is one
   more register, and a missing key reads as the default. No migration needed.
3. **Pages have identity.** The public API names a page by `PageId`, a type
   distinct from `NodeId` and minted by the CRDT in the same way. Deleting a page
   is one tree delete, and its objects go with it. Moving an object to another
   page, when a story asks for it, is a tree move that keeps the object's
   `NodeId`.
4. **There is always at least one page.** Commands refuse to delete the last
   page. Two peers can still each delete a different page of a two-page
   document at the same time, so an import that leaves no page appends one
   default page in a repair commit. `Document` never exposes zero pages.
5. **The active page is ephemeral** (ADR 0009 §2). It belongs to the same
   class as the view and the selection: it answers "what am I looking at", it
   is per peer, never written and never undoable. New and Open start on the
   first page. A held `PageId` is resolved lazily. If a peer deleted that
   page, the session moves to an adjacent one. Remembering the last page per
   device, if a story asks for it, is per-peer data outside the shared
   document, which is the same answer slice 4 gave for the view.
6. **Older files migrate on open.** `vecmanf-document-core` creates one page
   with the old root size and moves the old root objects under it in their
   order, so every `NodeId` is kept. It then removes the old size keys. All of
   this is one commit, made before the editing session starts. This is the
   first migration that writes rather than reading a missing key as a default.
   It writes because Loro tree positions cannot be reinterpreted in place.

## Consequences

- Every read that lists objects and every command that creates one takes a
  `PageId`. Commands that act on given `NodeId`s (translate, delete, edit)
  are unchanged, because ids are unique across the whole tree.
- Exporters (SVG, later jobs) take a page. How a multi-page project maps to
  SVG is decided by the SVG round-trip story.
- Layers (ADR 0002 §8) will sit under a page if they are tree nodes, which
  gives per-page layers. Layers shared across pages would have to be a
  reference register rather than a tree parent. `layers-and-grouping`
  inherits this choice and should make it explicitly.
- Reopening a project always shows its first page.
- Migration hazard, accepted: if two devices migrate the same old file
  separately and later sync, the merged document has two pages and one of
  them is empty. No file has been released, sync does not exist yet, so no
  such pair can exist.
- `format_version` is bumped by the slice that builds this, under the
  existing rule: the merging PR takes `main`'s `CURRENT_FORMAT_VERSION + 1`.
