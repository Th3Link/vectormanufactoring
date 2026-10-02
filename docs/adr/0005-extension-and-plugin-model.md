# ADR 0005: Extension and plugin model

**Status:** Accepted (customer sign-off, 2026-10-02)

Supersedes the deferral in the first draft. The customer's direction:
*"plugins will of course be written in Rust. Plugins are an essential
component."* Plugins are therefore day-one scope, Rust-first, and the previous
"publish nothing until two real plugins exist" position has been reconsidered
below.

## Context

Extensibility is one of the two named reasons this project exists: the
customer's complaint about Inkscape is its UI/UX "especially extensibility".
Against that, a *published* plugin interface is the single hardest thing in a
project to change later — once third parties ship against it, every mistake is
permanent.

The first draft resolved that tension by deferring: build the seam, publish
nothing. "Essential component" makes deferral the wrong answer, but it does not
make *freezing early* the right one. The distinction this ADR rests on:

> Shipping a plugin host early and freezing a plugin interface early are two
> different decisions. The first is now required. The second is still the
> expensive mistake, and it is avoided by declaring the interface unstable
> with a named freeze trigger — not by withholding it.

### Options considered

**A. Inkscape's model: an external process, document on stdin, document on
stdout.** Trivial and language-agnostic. Rejected as the plugin model: process
startup per invocation plus a full document serialization round trip is exactly
the slowness being complained about, it cannot work in the browser, and it has
no sandbox. Its remaining value is *interop with foreign tools*, which is a
different feature — see §6.

**B. Native dynamic libraries (`cdylib`).** Fastest and fully capable, and
superficially the obvious choice now that plugins are known to be Rust.
Rejected anyway, and the "plugins are Rust" fact does not rescue it: Rust has
no stable ABI, so every plugin would break on every compiler upgrade unless
everything is rebuilt in lockstep; loading requires `unsafe` (`CLAUDE.md` §5
needs an ADR for that, and this ADR declines to grant it); there is no sandbox,
so a buggy plugin takes the editor and the open document with it; and it does
not exist in the browser, which would mean two plugin models.

**C. An embedded scripting language (Rhai, or Lua via `mlua`).** Low authoring
barrier. Rejected as the primary model now that the authoring language is known
to be Rust: it would mean designing and maintaining a second API surface for an
audience of nobody. Weak for compute-heavy work (a vectorizer, a stitch
generator) besides.

**D. WebAssembly component plugins** (`wasmtime` on desktop, the engine's own
runtime in the browser) behind a versioned WIT interface. Chosen. Sandboxed by
default with no filesystem or network unless granted; no `unsafe`; no ABI
breakage across compiler versions; fast enough for real work; and the *same*
model on desktop and in the browser, which matters because `CLAUDE.md` §6
already makes every core crate build for wasm32 and ADR 0001 now runs the whole
editor core as wasm in the webview. Rust compiles to wasm32 natively, so
"plugins are written in Rust" and "plugins are wasm components" are the same
sentence. Cost: `wasmtime` is a large desktop dependency, and the interface has
to be designed rather than grown.

## Decision

1. **Model: WebAssembly component plugins behind a versioned WIT interface,
   with a first-party Rust SDK.** Rust is the supported authoring language: we
   ship `vecmanf-plugin` (a crate wrapping the `wit-bindgen` output in ergonomic
   Rust types — `Length`, `Point`, `Path` and the rest from ADR 0002, not raw
   WIT records) plus a `cargo generate` template and a worked example plugin.
   Language-agnosticism is a free side effect of wasm, not a goal we spend
   effort on.
2. **The plugin host ships in the MVP, not after it.** The interface is
   designed and the host built alongside the first machine family, so that at
   least one shipped capability — the first path optimizer or the first
   post-processor — *runs through the plugin interface in the shipping product*.
   A plugin interface that nothing we ship uses is an interface we have not
   tested.
3. **The interface is explicitly unstable until a named trigger, and the
   instability is published rather than implied.** Versions are `0.x`; breaking
   changes are allowed, listed in a `CHANGELOG` for the interface, and each
   `0.x` bump states the migration. The interface is frozen at `1.0` by a
   follow-up ADR when both hold: (a) the customer has written and shipped at
   least one plugin against it without our help — because the interface must
   survive someone who does not know the internals — and (b) two consecutive
   `0.x` releases have needed no breaking change. After `1.0`, additions only.
   This replaces the first draft's "publish nothing": users get plugins
   immediately and get told what the stability promise is.
4. **The seam is built from the inside, as before.** Plugin-shaped capabilities
   are pure functions in core crates with the exact shape the WIT mirrors:
   **paths in, paths out**, plus a declared parameter set. Vectorizer variants,
   path optimizers, fill/stitch generators and machine post-processors are
   written this way whether or not a plugin ever calls them. A capability that
   cannot be expressed as "document subset in, document subset out, parameters
   declared" is not plugin material.
5. **Plugin edits go through the command journal** (ADR 0002 §9), never into
   the document directly, so no plugin can corrupt a document in a way undo
   cannot reverse. With collaboration (ADR 0004) this gains a second clause:
   **a plugin runs on the peer that invoked it, and only its resulting commands
   replicate.** Plugins are never re-executed on other peers — we do not
   require plugin determinism, and we do not require every peer to have the
   plugin installed to receive its results.
6. **The external-command interop feature is demoted and decoupled from the
   plugin story.** A user-configured command that receives an SVG export and
   reads back SVG or a machine file remains worth having — it is how you call
   Ink/Stitch or a vendor tool you do not control — but it is *interop with
   foreign software*, not a plugin mechanism and not a fallback for one. Since
   plugins are now essential and shipping, this is no longer on the critical
   path: it becomes its own story, scheduled on its own merits.
7. **Plugins get no network and no filesystem at MVP.** The sandbox denies
   both; nothing grants them. In particular, **asset-library connectors are not
   plugins** (ADR 0007) precisely because they need network access and
   credential storage, which the plugin sandbox exists to withhold. A
   capability-grant model for plugins is a separate ADR, triggered by a story,
   and will need a user-facing consent surface before it is written.
8. **Plugins declare parameters; the application renders the controls.** No
   plugin draws its own UI. Under ADR 0001 this is now a strength rather than
   just a constraint: a declared parameter schema renders as generated React
   form controls built from the same design system as the rest of the app, so
   plugin dialogs cannot look or behave like the Inkscape-extension dialogs the
   customer is escaping. "A plugin with its own panel" needs a new ADR.
9. **No plugin trait, registry or capability enum is added before the host
   story.** When the host is built, the registry and the capability enum are
   written concretely for the capabilities that exist — not speculatively for
   ones that might (`CLAUDE.md` §5).

## Consequences

- The headline customer pain gets a real answer in the first release instead of
  a roadmap entry, and the customer can write plugins as soon as the MVP ships.
- **We pay for the plugin host earlier than the editor strictly needs it.**
  `wasmtime` adds significant build time and desktop binary size, and §2's
  "ship a real capability through the interface" is extra work on the MVP's
  critical path. That is the price of "essential component" and it is accepted
  deliberately; the browser build pays nothing, since it uses the host engine.
- **`0.x` means early plugins will break.** Anyone writing against the
  interface in the first months must expect to update. The alternative — a
  frozen interface designed before any real plugin existed — is worse and
  permanent. The freeze trigger in §3 is what makes this honest rather than
  open-ended, and the customer writing a plugin is on the critical path to
  `1.0`.
- The Rust SDK is a maintained public API surface in its own right: semver,
  docs, an example that must keep compiling in CI.
- Writing core features as "paths in, paths out" costs some convenience (no
  reaching into application state from a vectorizer). It is cheap now and
  expensive to retrofit, which is the whole reason for deciding it early.
- Plugins cannot read files or reach the network, so "plugin that fetches
  something" is impossible until the capability ADR exists. This will come up,
  and the answer is an ADR, not a quick grant.
- AGPL-3.0 (ADR 0006) applies to plugins linked against this interface. With
  the customer's "there will be no paid plugins", that is consistent; it does
  foreclose a proprietary third-party plugin ecosystem, which ADR 0006 flags
  for explicit confirmation.
