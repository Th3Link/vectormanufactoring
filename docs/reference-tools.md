# Reference tools

Software the customer looks at as competition or as a source of ideas: what
it does, what we take from it, what we avoid. Entries are the customer's
observations, not feature audits. Add a line when a new tool comes up; do not
copy designs from it, learn from how they solved a problem.

| Tool | What it is | What we take | What we avoid |
|---|---|---|---|
| Inkscape | Free vector editor, the drawing baseline of the maker workflow. | Node/handle/segment mental model, Trace Bitmap quality as the bar, plain SVG interop. Layers and Objects dialog (specs 0050): a tree with eye and lock on each row, rename by double-click, drag to reorder, Shift+Ctrl+L. Clone menu (spec 0049): clone on top of the original, select original, unlink. | Slow, hard-to-discover extensions; a crowded interface. A layers dock that takes canvas width (ours is a tab of the Properties panel); object lists reported slow with thousands of objects (to check against the customer's version); clones that can form chains and become orphans. |
| LightBurn | Laser job software. | Job and material handling for lasers, the direct-send idea. Virtual Array (spec 0049): synced copies of one source, flattened at output. | Linux support ended with 1.7.x; no cross-vendor material library. Virtual copies that cannot be selected or changed on their own; layers as a colour palette with no tree and no object list. |
| Ink/Stitch | Inkscape extension for embroidery. | Embroidery as part of the same project, not a separate tool. | Extension-only integration. |
| VectorCraft | Vector editor from the ArtCraft suite (Rust, MIT or Apache-2.0, in development; v0.3.1 per the [Clubic listing](https://www.clubic.com/telecharger-fiche633055-vectorcraft.html); repository link to be added). Ambitious, copies Adobe Illustrator closely (customer, 2026-10-09). | Respectable scope and ambition; worth studying how they solved booleans, panels and tools, and how a pure-Rust vector editor is built. | The interface: cluttered and overwhelming, it imitates Illustrator instead of finding its own, simpler way. |
| Adobe Illustrator | Industry-standard vector editor. | UX/UI reference (customer). Layers panel with an eye and a lock per row (spec 0050); symbol instances that follow a master and can be transformed alone (spec 0049). | Sublayers, the target circle and a colour per layer. |
| Affinity (Designer) | Vector and raster editor. | UX/UI reference (customer); the Curve and arc handles on ellipse, polygon and star (spec 0021) follow it. Layers panel with the opacity of the selected row above the list (spec 0050). | |
| Blender | 3D suite. | UX/UI reference (customer). | |
| GIMP | Raster editor. | UX/UI reference (customer); value fields (drag inside the field, or type a number). Not wanted: +/- buttons, the 0..100 and 0..255 toggles, the restless colour dialog. | |

## What this means for Curvyo

The customer is not satisfied with the user interface of the Illustrator-style
editors: too many controls at once. Curvyo keeps the screen calm: few controls
visible, details when something is selected, nothing in the properties panel
that does not apply (see `docs/design-system.md`, "Properties panel" rules,
and `specs/0017-style-panel-rework/`).
