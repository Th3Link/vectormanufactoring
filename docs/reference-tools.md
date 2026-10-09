# Reference tools

Software the customer looks at as competition or as a source of ideas: what
it does, what we take from it, what we avoid. Entries are the customer's
observations, not feature audits. Add a line when a new tool comes up; do not
copy designs from it, learn from how they solved a problem.

| Tool | What it is | What we take | What we avoid |
|---|---|---|---|
| Inkscape | Free vector editor, the drawing baseline of the maker workflow. | Node/handle/segment mental model, Trace Bitmap quality as the bar, plain SVG interop. | Slow, hard-to-discover extensions; a crowded interface. |
| LightBurn | Laser job software. | Job and material handling for lasers, the direct-send idea. | Linux support ended with 1.7.x; no cross-vendor material library. |
| Ink/Stitch | Inkscape extension for embroidery. | Embroidery as part of the same project, not a separate tool. | Extension-only integration. |
| VectorCraft | An ambitious vector editor that copies Adobe Illustrator closely (customer, 2026-10-09). The link and the exact project are to be confirmed: at least two unrelated projects carry this name (an ArtCraft app written in Rust, and a web SVG icon editor). | Respectable scope and ambition; worth studying how they solved things such as boolean operations, panels and tools. | The interface: cluttered and overwhelming, it imitates Illustrator instead of finding its own, simpler way. |

## What this means for Curvyo

The customer is not satisfied with the user interface of the Illustrator-style
editors: too many controls at once. Curvyo keeps the screen calm: few controls
visible, details when something is selected, nothing in the properties panel
that does not apply (see `docs/design-system.md`, "Properties panel" rules,
and `specs/0017-style-panel-rework/`).
