# Project file foundation: new, open, save a local project

Status: Done
Priority: Must
Origin: Customer

## User value

As a maker I want to create, save and reopen a project file on my own
computer, with nothing required beyond the app itself, so that my work is
mine from the first click — before I've drawn anything, and without the app
needing a network connection or an account to let me start.

This is the thinnest possible slice: an empty canvas that survives being
saved, closed and reopened. Every later editing feature (drawing, styling,
layers, job output) needs this to exist first, and nothing in this slice
depends on any of them.

**What we do differently from the tools this replaces:** Inkscape has no
project container — the file *is* the SVG, so nothing but geometry can live
in it. LightBurn has a project container (`.lbrn2`) but it is Windows-only
and keeps no path to syncing or sharing that container later. Our `.curvyo`
project file (ADR 0004 §1) is a real container from the first save — a zip
holding the document plus room for jobs, assets and (later, unused in this
slice) a sharing keyring — so nothing about today's "just save a file"
story has to be redesigned when sync or collaboration slices land.

## Acceptance criteria

1. Given the app has no project open, when it is launched, then an empty
   canvas is visible within 3 seconds on ordinary desktop hardware, with no
   file dialog or network wait blocking that view.
2. Given no project is open, when the maker starts a new project (menu
   action or the app's default launch state), then an empty canvas is shown
   with rulers/status bar reporting position and size in millimetres (the
   canonical unit, ADR 0002 §2), and no file has yet been written to disk.
3. Given an unsaved new project, when the maker chooses "Save As" and picks
   a filename and folder, then a file with extension `.curvyo` is written at
   that exact path; the file is a valid zip archive that contains at least
   `document.loro` and `document.json`; and the app's title bar shows the
   chosen filename with no "unsaved" marker immediately after.
4. Given a project already saved once, when the maker chooses "Save" again
   having made no further changes, then the app completes without error and
   without re-prompting for a filename or location.
5. Given a `.curvyo` file previously saved by this slice, when the maker opens
   it (via "Open Project" or the OS file association/double-click), then the
   app shows an empty canvas with no error, within 3 seconds, and the title
   bar shows that file's name.
6. Given a saved project, when the maker closes the app entirely and
   relaunches it, then opening that same file reproduces the same empty
   canvas with no error — proving the round trip survives a full process
   restart, not just an in-session close.
7. Given a file that is not a valid `.curvyo` project (e.g. a renamed empty
   text file, a truncated zip, or a `format_version` newer than this build
   supports), when the maker tries to open it, then the app refuses with a
   specific, named error message (not a silent no-op, not a crash) and any
   other already-open project is left untouched.
8. Given the machine's network connection is fully disabled, when the maker
   performs criteria 2, 3, 5 and 6 in sequence, then every step completes
   successfully with no error that mentions connectivity — proving R-SYS-002
   ("full single-user functionality offline") holds for this slice.
9. Given the app is used normally with criteria 2–6 while the machine *is*
   online, when network traffic is monitored for the duration, then zero
   outbound connections are made — proving no telemetry leaves the machine
   by default (R-SYS-007) even though nothing stops it from being online.
10. Given the same `.curvyo` file produced by criterion 3, when it is opened on
    a Linux, a Windows and a macOS build of the app, then each one opens it
    without error and shows an empty canvas (R-SYS-001: one shared
    codebase, all three platforms). Pixel/geometry-exact parity across OSes
    is R-SYS-003's job, deferred past MVP (`docs/requirements.md`); this
    criterion only requires "opens without error" on all three.

## Out of scope

- Any drawing or editing tool (paths, primitives, styling, layers) — slices
  2–7 in `specs/README.md`.
- Unsaved-changes tracking and a "save before closing?" prompt. An empty
  canvas has nothing to lose yet; dirty-state tracking belongs with the
  first slice that can actually dirty the document (`path-node-editing`,
  built on the command journal of ADR 0002 §9).
- Recent-projects list, a welcome/start screen, project templates, multiple
  open projects or windows/tabs. Possible later UX proposals, not needed to
  try this slice.
- Auto-save and crash recovery. Not decided; a `Proposal` for a later round
  if the customer wants it.
- Any machine or material binding for the project (work-area size, etc.) —
  `machine-profile` (slice 10).
- Collaboration, cloud sync, folder sync, and the `keyring.log` sharing
  mechanism inside the `.curvyo` container. The container format already has
  room for these (ADR 0004 §1, ADR 0010) so later slices are additive, but
  none of it is exercised, wired up, or user-visible here.
- The explicit "recover from `document.json` snapshot" action named in
  ADR 0004 §1. The file format supports it; exposing it as a maker-facing
  recovery flow is a later story.

## UX notes

This is the first feature with any UI at all, so several of these decisions
are precedent for everything that follows, not just local choices. Flagged
below where that matters. `docs/design-system.md` does not exist yet; this
slice does not need it (one background colour, one status bar, OS-native
controls for everything else), but the first feature that needs real tokens
(palette, spacing scale, type scale) should seed that file rather than
inventing tokens inline the way this spec does for its one colour.

### Window chrome and menu (precedent)

- **Native OS window decorations and a native OS application menu** (Tauri's
  `Menu`/`MenuBuilder` API), not a custom in-canvas menu bar or a custom
  title bar. This is the Affinity/Inkscape convention, it is free
  accessibility and keyboard traversal (screen reader menu navigation, full
  keyboard nav) on every platform, and it costs no widget code. Revisit only
  when a story needs chrome a native menu cannot provide (e.g. a tool
  palette that wants a custom title row) — no story does yet, so don't
  pre-build one (`CLAUDE.md` §5).
- One top-level menu for this slice: **File** → New, Open…, Save, Save As…,
  separator, Quit. Do not stub empty Edit/View/Help menus "for later" — add
  each exactly when a story first needs it.
- Below the menu, the entire window body is canvas. No toolbar, no side
  panels, no breadcrumb — none exist yet.

### Canvas and empty state

- Canvas fills the window body edge-to-edge: no margin, border, or card
  frame around it. **Precedent: chrome never frames the canvas** — later
  panels dock beside/over it, they don't shrink it into a bordered box.
- Empty canvas is just the canvas background colour (`--canvas-bg`, a single
  CSS variable even though only one theme exists yet — don't hardcode the
  hex value inline, so the eventual light/dark ADR work is a variable swap,
  not a search-and-replace). No placeholder illustration, no "drop a file
  here" hint, no call-to-action — it should read as "ready", not "empty/
  broken".
- **Status bar**: a thin bar pinned to the bottom edge, full width, ~24px
  tall, background `--statusbar-bg` (low-contrast against canvas but
  distinct). Left side: cursor position in mm (`x: 0.0  y: 0.0`). Right
  side: document size in mm. This is the one permanent piece of chrome this
  slice adds (satisfies AC2's "rulers/status bar"). Leave room for a zoom
  control later but do not add a non-functional one now.
- No splash screen and no intermediate loading spinner — AC1's 3-second
  budget is tightest right at launch, and a splash screen would spend part
  of that budget on itself for no benefit on an empty canvas.
- Default window size 1280×800, resizable. Remembering window size/position
  across launches is a nice later touch, not required by any acceptance
  criterion — don't build persistence for it in this slice.

### Title bar states

- No project open / unsaved new project: title is **"Curvyo"** (app name
  only — not "Untitled", there's no recent-files or template concept to
  distinguish it from yet).
- After Save As or Open: **"<filename> — Curvyo"** (em dash). Never show
  the full path in the title bar — path lives only in the native file
  dialog and, later, a recent-files list.
- No unsaved-changes marker anywhere in this slice (AC3 requires its
  *absence* immediately after Save As) — there is no dirty-state tracking
  to drive one yet (see Out of scope).

### Keyboard shortcuts (from day one)

Bind these as accelerators on the native menu items (not a separate
app-level keymap table) so OS-standard discoverability comes for free:

| Action | Windows/Linux | macOS |
|---|---|---|
| New | Ctrl+N | Cmd+N |
| Open | Ctrl+O | Cmd+O |
| Save | Ctrl+S | Cmd+S |
| Save As | Ctrl+Shift+S | Cmd+Shift+S |

### File dialogs

- Open and Save As use the **OS-native file picker** (Tauri's `dialog`
  plugin), not a custom in-app browser — gives platform-correct recent
  locations and search for free, matches every reference tool.
- Save As default filename: `Untitled.curvyo` when the project has never been
  saved.
- Register the `.curvyo` file association and the native picker's filter as
  "Curvyo project (*.curvyo)" (needed for AC5's double-click/open-with path).

### Error handling — invalid/corrupt file (AC7)

- Use shadcn/ui **`AlertDialog`** (not a toast, not a browser `alert`, not
  the plain `Dialog`): this is a blocking failure the maker must
  acknowledge, and `AlertDialog` is the shadcn primitive that doesn't
  dismiss on outside-click.
- Title: **"Can't open project"**. Body is one plain-language sentence
  naming the specific cause, never the raw error type:
  - Not a `.curvyo` at all / not a zip: *"This file isn't a Curvyo project
    (.curvyo) file."*
  - Truncated/corrupt zip or missing required members: *"This file is
    damaged and can't be read."*
  - `format_version` newer than this build: *"This file was saved by a
    newer version of Curvyo. Update the app to open it."*
- Single "OK" action, default-focused; Escape and Enter both dismiss.
- The dialog is modal only to the open attempt. If another project is
  already open behind it, dismissing the dialog must leave that project's
  canvas exactly as it was (AC7's "already-open project is left
  untouched") — the open attempt must not touch app state until it
  succeeds.
- No "retry" or "report a bug" affordance here — not asked for, don't add
  error-recovery UI beyond naming the problem.

### First launch, no recent files

- First launch is visually identical to "new project" (AC1 reduces to
  AC2): empty canvas, status bar, title "Curvyo". No welcome screen, no
  empty-state illustration, and no greyed-out "Open Recent" stub — recent
  files are explicitly out of scope; don't build a disabled placeholder for
  a feature that doesn't exist.

### Precedent set here for later features

Native OS menu over custom menu/title bar; canvas always edge-to-edge under
chrome; status bar as the one permanent chrome element; shadcn `AlertDialog`
as the house style for blocking errors; `"<filename> — Curvyo"` title
format. Later feature UX notes should follow these unless they explicitly
supersede one.

## Links
Requirements: R-SYS-001, R-SYS-002, R-SYS-007 (`docs/requirements.md`)
ADRs: ADR 0002 (document model, units), ADR 0004 §1 (`.curvyo` container
format)
PR: https://github.com/curvyo/curvyo/pull/3
