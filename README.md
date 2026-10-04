# vecmanf

A desktop-first application for makers who draw, prepare and produce vector
work on laser cutters, cutting plotters, embroidery machines and CNC mills —
one tool instead of a chain of four.

## Status

Pre-code. Requirements and the foundational architecture decisions are
settled: no crates exist yet. See `docs/requirements.md` for what the system
must do and [`docs/adr/index.md`](docs/adr/index.md) for the ten accepted
ADRs covering UI framework, document model, geometry kernel, persistence and
sync, the plugin model, license, asset connectors, end-to-end encryption and
concurrent editing. Nothing here is ready to run.

## Getting started

```sh
cargo install tauri-cli --locked
cd frontend && npm ci
cargo tauri dev     # or: cargo tauri build
```

## Why

The motivating case is a maker who currently chains four separate tools to
go from an idea to a finished physical object:

1. **Inkscape** to draw and edit vector graphics.
2. **Ink/Stitch** (an Inkscape extension) to turn vector art into embroidery
   stitch paths.
3. A **cutting plotter's** own export path to cut or draw the design.
4. **LightBurn** to drive a laser cutter for cutting and engraving.

Each tool is good at one step and knows nothing about the others. Material
settings, test cuts and finished designs live in four different places (or
nowhere at all), and nothing carries over when the maker switches machines
or computers. Concretely, this project exists to fix:

- **Inkscape's UI/UX and extensibility.** Extensions are slow, hard to
  discover, and feel bolted on rather than built in.
- **LightBurn's Linux situation.** As of LightBurn 1.8, Linux is no longer
  supported at all (1.7.x is the last Linux-capable release) — see the
  vendor's own forum announcement. LightBurn also has no concept of a
  material library that spans machines, and no workflow that connects to
  the design/editing step.
- **No cross-machine, cross-material memory.** Every laser/material
  combination needs a power/speed test cut. Today that knowledge either
  lives in the maker's head, a notebook, or is re-discovered by trial and
  error on a second machine. It should be recorded once per machine and
  material, and reused — and it should follow the maker across computers,
  not just live on one machine's LightBurn install.

## What this is

- A single desktop application (Linux, Windows, macOS first; browser
  second; mobile companion later — see `CLAUDE.md`) that covers the whole
  path from drawing to machine output for one job.
- A vector editor with the path/node editing a maker actually needs day to
  day: Bézier paths, primitives, boolean operations, stroke/fill styling,
  object-to-path.
- A manufacturing tool with first-class support for laser cutting and
  engraving, cutting plotters, embroidery machines, and CNC milling
  (2.5D and V-carving), each producing the machine's real output format
  (G-code dialects, HPGL, embroidery formats such as DST/PES/EXP).
- A material and machine memory: test-cut patterns and their results are
  stored per machine and material, and sync across the maker's computers.
- A font manager that lets a maker work with a large, curated font
  collection — categorized by manufacturing suitability (cuttable
  single-line scripts, initials, symbols/ornaments) — without installing
  every font system-wide or cluttering every other application's font
  list.
- An asset manager for reusable templates and finished designs, with room
  to add further asset libraries later, including third-party libraries the
  app connects to via a maker-supplied API token or login — we don't run
  accounts or payments ourselves, that's the asset provider's business. The
  connection is read *and* write: where the maker's credential grants
  upload rights on that service, the maker can push their own drawings to
  it from inside the app, not just browse and download. The first
  read-oriented connector target is [Iconify](https://iconify.design) for
  icon/symbol/ornament assets; the first read+write target, for pushing a
  maker's own files, is a **git forge** (via a repository the maker already
  has write access to). A usable third-party source for fonts specifically
  has not been identified yet (open research item).
- Real-time collaborative editing: two makers on two instances can work in
  the same project at once, backed by a small server, with cloud sync as
  the standing cross-computer mechanism and folder-based sync (e.g.
  Nextcloud) as an additional offline-friendly option. Collaboration and
  cloud-sync data is end-to-end encrypted: the server operator, including a
  self-hoster, cannot read document content. A document can have more than
  one admin; an admin can invite, remove and re-add collaborators, and a
  removed collaborator keeps their last-seen local state but gets no
  further updates and can't push new ones.
- A Rust plugin interface as a core part of the architecture, not an
  afterthought bolted on like Inkscape's extensions.

## What this is not

- Not a general-purpose illustration or raster image editor. Vectorization
  (bitmap-to-vector tracing) is in scope because makers need it to get
  artwork onto a machine; photo editing is not.
- Not a slicer for FDM/resin 3D printing. CNC milling here means 2.5D
  routing and V-carving from vector/toolpath data, not additive
  manufacturing.
- Not a device firmware or controller board project. We generate and send
  machine-ready output (G-code, HPGL, embroidery formats); we do not build
  or flash firmware.
- Not an accounts-and-payments platform. We connect to third-party
  asset-library services with credentials the maker supplies; running
  logins, subscriptions or billing is the asset provider's job, not ours.

## Project structure

This is a Rust workspace. No crates exist yet — see `CLAUDE.md` for the
intended core/platform split and `docs/guides/rust-workspace-blueprint.md`
for the layout conventions this project follows once code lands.

## Docs

- [`docs/requirements.md`](docs/requirements.md) — system-level
  requirements and the accepted MVP cut.
- [`docs/adr/index.md`](docs/adr/index.md) — architecture decision records;
  all ten are accepted.
- `specs/` — feature specs, spec-driven (to be added now that requirements
  and the foundational ADRs are accepted); see [`specs/README.md`](specs/README.md)
  for the convention and [`specs/index.md`](specs/index.md) for the MVP's
  feature sequence.

## License

AGPL-3.0-or-later — see [ADR 0006](docs/adr/0006-license.md) and the
`LICENSE` file at the repo root.
