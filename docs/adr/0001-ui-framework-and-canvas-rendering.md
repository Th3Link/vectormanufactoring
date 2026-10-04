# ADR 0001: UI framework and canvas rendering

**Status:** Accepted (customer sign-off, 2026-10-02)

Supersedes the egui recommendation in the first draft of this ADR. The
customer's direction: *"we are switching to Tauri or Dioxus plus Tailwind, to
make better use of `ui-ux-pro-max`. UX/UI is what everything stands or falls
on."* That fixes the family (web-tech-rendered, Tailwind-stylable) and sets the
priority bar (achievable UX quality ahead of engineering purity). The choice
between Tauri and Dioxus was delegated to the architect and is made below.

## Context

The application needs a dense, professional editor shell (tool palette, layer
and job lists, property panels, material browser, font browser) plus one
high-performance canvas: pan and zoom over documents with thousands of path
nodes, live node dragging, toolpath and engraving preview. It must run on
Linux, Windows and macOS first and in the browser second (`CLAUDE.md` §6).
Every `*-core` crate must build for `wasm32-unknown-unknown`, so the UI layer
must not leak into core.

Two further facts now shape the choice:

- **UX quality is the acceptance bar.** The reason the project exists is that
  Inkscape's UI/UX and extensibility are unacceptable to the customer. A shell
  that is merely functional fails the brief.
- **`ui-ux-pro-max` is the design-intelligence source we have.** It ships 22
  stack-specific data sets. `react`, `shadcn` and `html-tailwind` are among
  them; **no Rust GUI stack is**, and neither is Dioxus. Its stack layer is
  usable only by a DOM + Tailwind + React-shaped frontend.

### Options considered

**A. Tauri 2 — Rust host process + a TypeScript/React/Tailwind frontend, with
the editor core compiled to wasm and loaded *into* the frontend.** Chosen; see
Decision. The crucial point is the third clause: the frontend does not call the
Rust host for editing. `vecmanf-document-core`, `vecmanf-ui-core` and
`vecmanf-render-core` are compiled to `wasm32-unknown-unknown` — which
`CLAUDE.md` §6 requires anyway — and run in the webview next to the UI, so
drag-driven direct manipulation never crosses a process boundary. The Tauri
host is used for what only it can do: filesystem, dialogs, device
communication, auto-update. This also makes the desktop and browser targets the
*same* frontend plus the same wasm module, with a different I/O backend behind
one narrow interface.

Costs, named honestly: a second language and an npm dependency tree; two build
systems and two quality gates; and a wasm↔JS boundary that has to be designed
(see Decision §5) or it becomes the performance bug.

**B. Dioxus 0.7 — Rust-only, RSX components, Tailwind via class strings.**
Rejected. It is the more tempting option on paper (one language, one gate, no
npm) and it loses on the customer's own stated criterion:

- On desktop, Dioxus renders in **the same system webview as Tauri** (WebKitGTK
  on Linux, WebView2, WKWebView). It therefore has *no* canvas or rendering
  advantage to trade against its ecosystem gap. The alternative native renderer
  (Blitz/Vello) is not production-ready and has no webview-grade text input,
  IME or accessibility.
- The dense widgets this editor needs — accessible combobox, virtualized tree,
  resizable panes, command palette, focus-trapped dialogs and menus, drag
  reorder — are exactly where a component ecosystem does the expensive work.
  React has Radix/shadcn, headless libraries and years of accessibility
  bug-fixing behind them. Dioxus has to have each one written from scratch in
  RSX, by us, and each one is a chance to ship a worse interaction than
  Inkscape's.
- `ui-ux-pro-max` has no Dioxus data set. Tailwind class strings transfer part
  of `html-tailwind`, but the `react`/`shadcn` component guidance does not.
  Since "make better use of `ui-ux-pro-max`" is the customer's stated *reason*
  for the switch, an option that only partly enables it does not serve the
  direction it came from.
- Dioxus' API has moved substantially between releases; its stability risk sits
  on the critical path of the thing we are told must not be mediocre.

The one real cost of rejecting B is the one named under A: a JS/TS layer. That
is a maintenance cost, not a UX ceiling, and it is paid in the layer that is
cheapest to replace.

**C. egui + eframe on `wgpu`** (the previous recommendation). Rejected by the
customer's direction and, independently, by the UX bar: visual polish,
accessibility and text/IME handling would all be ours to build from a lower
starting point, and no `ui-ux-pro-max` stack data applies.

**D. Iced.** Rejected for the same reasons as C, plus a thinner widget set for
dense professional panels.

**E. Slint.** Rejected: licensing (GPLv3 or a paid license) would constrain
ADR 0006 instead of following from it, and embedding a custom CAD canvas is
awkward.

**F. GTK4 via gtk-rs.** Excellent on Linux, materially worse elsewhere, no
browser path. Rejected against the stated target priority.

### The canvas, in each option

The drawing surface is a GPU canvas in every option; only the host differs.
Under A and B it is a `<canvas>` in the webview. `wgpu` compiled to wasm
targets **WebGL2** everywhere and WebGPU where the engine has it (WebView2 on
Windows: yes; WKWebView and WebKitGTK: not reliably). So the baseline is
WebGL2, and it is the same baseline in the browser target — one renderer, one
code path, verified once.

**This is the biggest technical risk in the decision**, and it belongs to Linux,
the customer's primary platform: Tauri on Linux uses WebKitGTK, the weakest of
the three engines for sustained canvas work. Recorded in
`docs/technical-debt.md`; it must be measured with a real stress scene before
the first canvas story, not assumed.

## Decision

1. **The decision that is hard to reverse (unchanged):** no UI framework type
   appears outside the shell. Interaction logic that is not widget code lives
   in `vecmanf-ui-core` — active tool state machines, selection, hit-testing
   against the document, snapping, command dispatch, undo driving — as plain
   state and pure functions, wasm-compatible, with no UI dependency. The
   frontend renders state and forwards input events. This boundary is worth
   *more* under a two-language stack than it was under a Rust-only one: it is
   what keeps the behaviour and its tests in Rust and the TypeScript thin.
2. **The shell is Tauri 2** with a **TypeScript + React + Tailwind CSS**
   frontend using **shadcn/ui** (copy-in components over Radix primitives).
   React and shadcn specifically, over Svelte or plain Tailwind, because that is
   where `ui-ux-pro-max`'s stack data and the accessible-component ecosystem
   both are.
3. **The editor core runs in the webview as wasm.** This ADR authorizes one new
   crate (`CLAUDE.md` §5): `vecmanf-editor-wasm`, a thin `wasm-bindgen` facade
   over `vecmanf-document-core`, `vecmanf-ui-core` and `vecmanf-render-core`.
   It contains no logic of its own — only binding, (de)serialization and the
   draw-list handoff. It is the only crate allowed to depend on `wasm-bindgen`.
4. **Canvas rendering:** the document is drawn in a dedicated WebGL2 context
   via `wgpu`-on-wasm, not with DOM elements or SVG.
   `vecmanf-render-core` turns the document plus a view transform into a flat
   draw list and tessellates it with `lyon`; it stays pure and wasm-compatible.
   GPU submission lives in `vecmanf-editor-wasm`. DOM overlays are allowed only
   for text input and for handles that must be accessible to a screen reader.
5. **The wasm↔JS boundary is an explicit design constraint, not an
   afterthought.** The document lives in wasm linear memory and is never
   mirrored in JS. Per-frame data crosses as typed arrays over shared memory
   views; per-interaction data crosses as small declared messages. No
   JSON-serializing the document or the selection per frame. Violations of this
   are a review finding, not a performance ticket.
6. **The Tauri host (`vecmanf-app`) owns only platform I/O** — file open/save,
   the data directory, device communication (ADR 0004, `*-io` crates),
   auto-update — behind one narrow command interface that the browser target
   implements differently. Nothing else is allowed in it.
7. No second UI target (mobile, alternative shell) gets code and no UI
   abstraction trait is introduced until a story needs it (`CLAUDE.md` §5).

## Consequences

- **The UX ceiling is the highest available to us**, and `ui-ux-pro-max`'s
  stack layer applies directly for the first time. Accessibility, text input,
  IME and font rendering come from the engine instead of being our project.
  This is the whole reason for the change.
- **`CLAUDE.md` §7's gate no longer covers the whole product.** The frontend
  needs its own gate — `tsc --noEmit`, ESLint, Prettier, `vitest`, and
  Playwright for interaction tests — run in CI alongside the Rust gate. §7 and
  §8 must be amended by the lead; until they are, a green Rust gate is not a
  green build.
- **Two dependency ecosystems to police.** `cargo deny` does not see npm.
  CI needs an npm license and audit check with the same allow-list discipline
  (ADR 0006 §2).
- **Linux/WebKitGTK canvas performance was the one unverified assumption** on
  the customer's primary platform, and the one failure that would have
  invalidated this ADR rather than cost a refactor. **Measured 2026-10-03:
  passed** — 50 000 nodes at a vsync-locked ~60 fps under sustained pan/zoom
  plus a live single-node drag, in a real WebKitGTK webview. §4 stands. The
  result and the two implementation requirements it imposes — disable
  WebKitGTK's DMA-BUF renderer on Linux, and reconfigure the `wgpu` surface on
  every resize — are recorded in `specs/0002-path-node-editing/adrs.md`; the two
  NVIDIA driver problems the measurement exposed are in
  `docs/technical-debt.md`.
- One renderer path (`wgpu` → WebGL2) serves desktop and browser, so the
  browser target stays genuinely close rather than nominally possible. The cost
  is that we design for the WebGL2 feature set, not WebGPU's.
- Three artefacts instead of one (Tauri host binary, wasm module, frontend
  bundle) means a more involved build, packaging and debugging story, and
  source maps plus wasm symbols have to be wired up deliberately.
- Replacing the frontend later (to Dioxus, or to a native shell) stays a rewrite
  of view code only, because of §1 — the same property the previous draft
  bought, for the same reason.
