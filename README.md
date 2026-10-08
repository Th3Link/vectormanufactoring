# Curvyo

Curvyo is one application to draw, prepare and produce vector work on laser
cutters, cutting plotters, embroidery machines and CNC mills.

- Desktop first (Linux, Windows, macOS), browser second, mobile companion
  later.
- Local-first and offline-capable. No telemetry.
- Licence: AGPL-3.0-or-later ([ADR 0006](docs/adr/0006-license.md)).
- Home: [curvyo.org](https://curvyo.org), source: `github.com/curvyo/curvyo`.

## Status

Early development. The app starts and you can draw and edit. You cannot
produce a machine job yet.

What works today:

- Create, open and save a local project file (`.curvyo`).
- Draw Bézier paths with the pen tool. Edit nodes and handles. Node types
  Corner, Symmetric and Asymmetric. Join two path ends and split a path at a
  node.
- Rectangle (with corner radius), ellipse, polygon and star tools. Convert
  any of them to a path ("object to path").
- Pan and zoom the canvas. Select, move and delete any object with the Select
  tool.
- Scale, rotate and skew a selection with on-canvas handles; typed values for
  angle, size, skew and move.
- Select by marquee or lasso, and pick between overlapping objects.
- A separate radius for each corner of a rectangle.
- Stroke width, dash, join, cap and colour, and fill. The properties panel is
  being reworked.

What does not exist yet:

- Undo and redo.
- Boolean operations.
- Groups and layers.
- SVG import and export.
- Raster trace.
- Machine profiles, cut/engrave roles, material records and test patterns.
- Job preview and G-code output.

The feature list with the status of every spec is at the top of
[`specs/README.md`](specs/README.md).

## Why

The motivating case is a maker who chains four tools to get from an idea to a
physical object:

1. **Inkscape** to draw and edit vector graphics.
2. **Ink/Stitch** (an Inkscape extension) to turn vector art into
   embroidery stitch paths.
3. A **cutting plotter's** own export path to cut or draw the design.
4. **LightBurn** to drive a laser cutter.

Each tool covers one step and knows nothing about the others. Material
settings, test cuts and designs live in four places or nowhere. Curvyo
targets three problems:

- **Inkscape's UI and extensibility.** Extensions are slow, hard to discover
  and bolted on.
- **LightBurn on Linux.** LightBurn 1.8 no longer supports Linux (1.7.x is
  the last Linux release). LightBurn also has no material library that spans
  machines and no link to the design step.
- **No memory across machines and materials.** Each laser and material pair
  needs a power/speed test cut. That knowledge sits in the maker's head or a
  notebook and is rediscovered on the next machine. Curvyo records it once
  per machine and material and reuses it.

## What it is

- A vector editor: Bézier paths, primitives, booleans, stroke and fill.
- A manufacturing tool for lasers, cutting plotters, embroidery and CNC
  (2.5D and V-carving), writing each machine's real output format (G-code,
  HPGL, embroidery formats).
- A material and machine memory: test patterns and results per machine and
  material.
- Later: font management by suitability, asset libraries with third-party
  connectors, end-to-end encrypted real-time collaboration, a Rust plugin
  interface. The architecture allows for these; none ship in the MVP.

## What it is not

- Not a general illustration or photo editor. Raster tracing is in scope.
  Photo editing is not.
- Not a 3D printing slicer. CNC means 2.5D routing and V-carving.
- Not firmware. Curvyo generates and sends machine output; it does not build
  or flash controller firmware.
- Not an accounts and payments platform. Third-party asset services use
  credentials the maker supplies; billing is the provider's job.

## MVP

The first release is laser only, local only, single user, one controller
dialect (GRBL-family G-code). It takes a design from drawing to a G-code
file for one laser and one material.

Deferred past the MVP: cutting plotter, embroidery and CNC; sync and
multi-OS file parity checks; fonts and assets; collaboration; the asset
connector; the plugin interface. See "MVP" and "Explicitly deferred past
MVP" in [`docs/requirements.md`](docs/requirements.md).

## Build and run

Prerequisites: the Rust toolchain pinned in `rust-toolchain.toml`, Node.js
with npm, the `wasm-bindgen` CLI at the version in `Cargo.lock`
(`cargo install wasm-bindgen-cli --version <version>`), `cargo install
tauri-cli --locked`, and on Linux the packages `libwebkit2gtk-4.1-dev`,
`libgtk-3-dev`, `librsvg2-dev`, `libayatana-appindicator3-dev`.

```sh
cd frontend && npm install && npm run build   # builds the wasm module, then dist/
cd ../curvyo-app && cargo tauri dev           # start the desktop app
```

Checks: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
-- -D warnings`, `cargo nextest run --workspace` (full gate: `CLAUDE.md` §7).

Not run for this README: `cargo tauri dev` and `cargo tauri build`. The npm
and check commands come from `frontend/package.json` and the CI workflow.

## Repository layout

Rust workspace at the repository root, layout fixed by
[ADR 0011](docs/adr/0011-workspace-and-crate-layout.md).

| Path | Role |
|---|---|
| `curvyo-document-core` | Document model, units, `.curvyo` container. |
| `curvyo-geometry-core` | Bézier segment operations (hit-testing, subdivision). |
| `curvyo-ui-core` | Tool state machines, hit-testing, selection, transforms. |
| `curvyo-render-core` | Draw-list builder: document and view to tessellated draw list. |
| `curvyo-editor-wasm` | Browser-facing facade over the core crates; owns the GPU surface. |
| `curvyo-storage-io` | Filesystem access for the desktop client. |
| `curvyo-app` | Tauri 2 desktop host: menu, file dialogs, file association. |
| `frontend/` | TypeScript, React and Tailwind UI shown in the Tauri webview. |
| `specs/` | One folder per feature: specification, ADR notes, plan. |
| `docs/` | Requirements, ADRs, design system, technical debt, guides. |

`*-core` crates have no I/O or UI dependencies and build for
`wasm32-unknown-unknown`.

## The project file

A project is one `.curvyo` file: a zip container holding the document as a
Loro CRDT snapshot plus embedded assets
([ADR 0004](docs/adr/0004-persistence-and-cross-machine-sync.md) §1).

## How it is developed

Spec-driven: each feature has a folder in `specs/` (specification with
numbered acceptance criteria, ADR notes, plan). Architecture decisions are
ADRs in `docs/adr/`. Workflow and rules: [`CLAUDE.md`](CLAUDE.md).

## Where to find things

- [`docs/requirements.md`](docs/requirements.md): requirements, priorities,
  MVP cut.
- [`docs/adr/index.md`](docs/adr/index.md): ADRs 0001 to 0013. Twelve are
  accepted; 0012 was rejected. ADRs 0001 to 0012 keep the old name
  `vecmanf`; read it as `curvyo` (ADR 0013).
- [`specs/README.md`](specs/README.md): the numbered list of all features
  with status, then the spec convention.
- [`docs/design-system.md`](docs/design-system.md),
  [`docs/technical-debt.md`](docs/technical-debt.md): UI design system, known
  issues.

## Contributing and licence

Branches are `story/<slug>`, `fix/<slug>` or `chore/<slug>`. Commits follow
Conventional Commits. `main` changes only through pull requests that pass CI
([`CLAUDE.md`](CLAUDE.md) §9). The repository is in English.

Licence: AGPL-3.0-or-later ([`LICENSE`](LICENSE)).
