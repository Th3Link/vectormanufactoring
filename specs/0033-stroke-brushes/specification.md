# Stroke brushes: a stroke that is drawn by a brush, with brushes the maker can add

Status: Draft
Priority: Could
Origin: Customer (brushes for the stroke, customizable, added by the customer's own code or extension, 2026-10-09). The kinds, the built-in set, the file format and every default below are my proposals until the customer has read them. A sketch, not a story to build yet; no `adrs.md` content yet.

## User value

As a maker I want to give a path a brush instead of a plain line (a tapered stroke, a calligraphic pen, a row of dots, a perforation, a scalloped edge), and to add brushes of my own without waiting for a release, so that decorative lines, borders and cut patterns are one stroke setting on a path and not a hundred hand-placed shapes.

The customer's words (translated): brushes for the stroke, customizable, either through an extension or through code, and addable by customers.

## What a brush is

A brush is a rule that turns the **centre line** of a path and its stroke width into the geometry that is drawn. The path stays an ordinary editable path; the brush is a stroke setting like the dash pattern. Four kinds, from simple to complex:

1. **Width profile** (calligraphic, tapered): the line's width varies along its length by a profile, for example thin at both ends and full in the middle, or a flat nib held at an angle. Result: one closed outline.
2. **Stamp** (scatter, dots): copies of a small shape placed along the path at a spacing, with a size, an optional rotation to follow the path, and optional seeded jitter. Result: many small closed shapes.
3. **Pattern along path** (art brush): one shape stretched and bent along the whole path, with optional different start and end pieces. Result: one deformed shape.
4. **Repeated tiles** (pattern brush with corner tiles): not proposed for the first version.

## Reference tools

- **Illustrator:** Calligraphic, Scatter, Art, Pattern and Bristle brushes in a Brushes panel; scatter and pattern brushes cannot use artwork with gradients or placed images; the brush is applied to existing paths and is editable until expanded ("Expand Appearance").
- **Affinity Designer:** vector brushes with pressure; stroke width profiles.
- **Inkscape:** no brush panel. The same results come from path effects (Pattern Along Path, PowerStroke, Bend) that live in a dialog, deform the pattern, and are slow and hard to find; this is the "extensions are slow and bolted-on" complaint behind ADR 0005. The Calligraphy tool draws a fixed outline and is not applied to an existing path.
- **LightBurn:** none.
- What we can do better: brushes are a stroke setting (one place), the list is in the panel with no dropdown, brush files are plain data anyone can write, brushes with parameters get their controls generated in the same style as every other field, and what is cut or engraved is exactly what is shown.

## Acceptance criteria (sketch; each gets numbers and tests before Ready)

1. **Picker in the Stroke section.** With a stroke on, the Stroke section has a Brush row: a grid of tiles, the first "None" (the normal stroke), then the built-in brushes, then the maker's own, each tile a small drawn sample of the brush along a short curve. No dropdown and no popup (`0017-style-panel-rework` panel rule). The pressed tile is the object's brush. How many tiles fit without an inner scroll area is a design question shared with `0022-color-management`.
2. **Brush parameters are generated.** A brush declares typed parameters (a number with a range and a unit, a choice from a list, a switch, an angle); the panel shows them as the standard rows under the picker (value fields for numbers, a segmented group for choices). A brush cannot draw its own controls (ADR 0005, decision 8). With brush None the rows are not in the tree.
3. **Width is the master size.** The stroke Width is the brush's nominal size: a profile's full width, a stamp's size, a pattern's height. Changing the width rescales the brush result. Colour and opacity are the stroke's; every brush paints in one colour.
4. **A built-in set.** Proposal for the first set, chosen for makers: Taper (width profile, both ends), Calligraphic (flat nib at a settable angle), Dots (round stamps), Perforation (short slits as stamps, for tear-off and cut lines), Scallop (pattern: a row of arcs), Zigzag (pattern, pinking edge). Final list is a question for the customer.
5. **Brush with dash, join, cap and markers.** While a brush other than None is set, the Dash, Join and Cap rows are not shown (hidden, not disabled): the brush owns the line's shape. Markers (`0018-stroke-markers`) stay available and are drawn on top at the centre line's start, end and nodes. Settings of hidden rows are kept and return with brush None.
6. **Brush shapes follow the path.** For stamps and patterns the shape is placed by arc length along the path, curves included, rotated to the path direction when the brush says so. Closed paths are repeated seamlessly around the corner. Test values (spacing, counts at a given length) come with the story.
7. **Reproducible.** Any randomness (size or position jitter, a scatter) comes from a seed stored with the stroke. The same document gives the same geometry on every machine and on every peer, and a job run twice cuts the same pattern. A "New random seed" action is the only way to change it.
8. **Limits.** A brush result has a maximum number of generated vertices (proposal: 1,000,000 per object) and stamp count (100,000). A brush that would exceed it is not applied; a notice says why and nothing is changed. Typical case: a dotted brush at a tiny spacing on a very long path.
9. **Expand brush.** A command turns the brush result into ordinary paths (closed outlines or the stamped shapes, with the stroke off and the fill on in the stroke's colour), so the maker can edit the result by node. One commit, `expand_brush`. Because there is no undo yet, the original is kept unless the maker chooses to replace it (Question 3).
10. **Manufacturing.** A brush stroke is **not design-only**: for laser, plotter, embroidery or CNC output the job uses the expanded geometry, so what is shown is what is cut or engraved. Roles (`manufacturing-roles`) and the job preview treat each generated outline like any path. Default and recommendation; Question 4 offers the opposite.
11. **Custom brushes: data files.** A brush is described in a plain text file (proposed format below), loaded by a pure function that takes the text and returns a validated brush or a list of problems, the same way as the presets of `0030-document-size-presets`. The maker adds a file by placing it in the app's brush folder or by an Import action using the operating system's file dialog; the brush shows in the picker without a restart. The folder and file reading are in a platform crate, the loader in a core crate.
12. **Custom brushes: code.** A brush that needs real computation (a procedural pattern, text along a path) is a plugin: a WebAssembly component built with the Rust SDK (ADR 0005) that takes the centre line, the width and its declared parameters and returns paths ("paths in, paths out", ADR 0005 decision 4). The plugin host is not part of the laser MVP (`docs/requirements.md`, R-EXT-001), so plugin brushes wait for it; data-file brushes do not. Built-in brushes use the same two routes, so the first built-in plugin-shaped capability keeps the interface honest.
13. **Brush in the file, with its definition.** The document stores a brush reference (id and parameter values) on the stroke and, for portability, a copy of the brush definition it uses (`brushes` table keyed by id and content hash), so a file opens and draws the same on a machine that does not have the brush. A plugin brush stores its definition's data and its last generated geometry as a cache; without the plugin the cache is shown and the brush cannot be regenerated (Question 5). This is a document-model and file-format change (`format_version` next free at merge).
14. **The library syncs like other libraries.** The maker's brush folder is a library in the sense of ADR 0004 §6 and §11, offline-first, synced with the maker's own computers. Brushes that arrive through an asset-library connector (R-ASSET) are a separate, later story.
15. **Validation.** Every shipped brush file is loaded by a test in the quality gate. A maker's file that fails to load is listed with each problem (line, reason) in the brush list, the other brushes still load, and nothing crashes.

## Proposed brush file format

TOML, like the presets file (Question 3 of `0030`: the architect decides TOML or JSON for both). A width profile and a stamp, to show the shape of it:

```toml
format = 1

[[brush]]
id = "taper"
name = "Taper"
kind = "width-profile"
# width factor along the path (0 = start, 1 = end), linear between points
profile = [[0.0, 0.0], [0.25, 1.0], [0.75, 1.0], [1.0, 0.0]]
cap = "round"

[[brush]]
id = "dots"
name = "Dots"
kind = "stamp"
shape = "M -0.5 0 A 0.5 0.5 0 1 0 0.5 0 A 0.5 0.5 0 1 0 -0.5 0 Z"  # unit box, SVG path data
follow_path = false

  [[brush.param]]
  id = "spacing"
  name = "Spacing"
  type = "number"
  unit = "stroke-widths"
  min = 0.5
  max = 20
  default = 2
```

Shapes are SVG path data in a unit box (1 = the stroke width); parameters are typed and ranged; nothing in the file can run code.

## Interplay with other features

- **Markers (`0018`):** decoration on the centre line, drawn over the brush result; they are not part of the brush output and are not expanded into it.
- **Dash (`0017`):** hidden while a brush is set (criterion 5). Perforation and Dots cover what a dash does with more control.
- **Pressure (`0032-pen-tablet-input`):** a width-profile brush is where recorded pressure naturally lands: the sampled pressure becomes the profile of that stroke. Both features need a width model on the path; they should share one document-model decision.
- **Variable-width stroke offset (`0038-path-offset`):** the `i_overlay` library already in the project can offset a stroke whose width varies along the path (round joins and ends only); the width-profile kind can start from it. No scope change here.
- **Booleans, offset, fracture, flatten, combine (`0016`, `0035`, `0037`, `0038`):** these work on shapes, not on strokes. A brush stroke is not an operand until it is expanded (criterion 9); they refuse it with a message that says so.
- **Paint None / stroke width 0:** no brush row, no brush drawn (`0017` criterion 5).

## Open questions (customer)

1. **Which kinds in the first version.** *A (default):* width profile and stamp. *B:* also pattern along path. *C:* all three. Recommendation: A, then pattern along path as a second slice; it needs the shape to be bent, which is the hardest part.
2. **Data files first, plugins later (criteria 11, 12).** Default: data-file brushes first; plugin brushes when the plugin host exists. Say if code brushes must come first.
3. **Expand: keep or replace the original.** Default: keep the original and add the expanded result as a new object (there is no undo yet). Option: replace, as Inkscape's "Object to path" does.
4. **Manufacturing (criterion 10).** *A (default, recommended):* the expanded geometry is cut. *B:* a brush is design-only and jobs cut the centre line, as markers may be (`0018` Question 5). B is surprising for Perforation and Scallop, whose whole point is to be cut.
5. **Plugin brush without the plugin (criterion 13).** Default: show the cached geometry, forbid regeneration, say so. Option: refuse to open such a file (bad: it locks the maker out of their own file).
6. **Colour of stamps and patterns.** Default: one colour, the stroke's. Multi-colour art brushes are out of scope.
7. **Make a brush from the selection** (Illustrator's "New Brush from selection"): default no, brushes are files. Say if you want it.
8. **When.** Default: not scheduled, after the laser MVP and after `0017-style-panel-rework`, `0018-stroke-markers` and the plugin host.

## Out of scope

- Raster, bristle, watercolour, pressure-dynamic simulation brushes.
- Multi-colour brushes, gradients inside brushes (`0017` removed gradients).
- Brushes for fill (hatches, patterns as a fill); this is a stroke feature.
- Brushes for text outlines or for primitives' strokes beyond what any stroke gets.
- A brush editor on the canvas, a brush marketplace, brushes delivered by an asset connector.
- Embroidery stitch brushes (running stitch, satin) as brushes: that is the stitch generator of R-MFG-EMB-001, which may later be offered through the same picker.
- Animating or stacking several brushes on one stroke.

## UX notes

(filled in by ux-engineer before Ready)

## Links

Requirements: R-EDIT-021 (`docs/requirements.md`); related R-EXT-001, R-EXT-002, R-EDIT-016, R-INP-001
Depends on: `specs/0017-style-panel-rework/`, `specs/0018-stroke-markers/`, the plugin host (ADR 0005, post-MVP)
Related: `specs/0030-document-size-presets/` (data-file loader), `specs/0032-pen-tablet-input/` (width model), `specs/0022-color-management/` (tile grid in the panel)
ADRs: `adrs.md` (architect, to come; the document model and plugin interface parts are `needs-customer`)
PR: TBD
