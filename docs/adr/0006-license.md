# ADR 0006: License

**Status:** Accepted (customer sign-off, 2026-10-02)

The customer decided: *"AGPL-3.0, good catch on the hosted-fork loophole.
There will be no paid plugins."* This revision changes the Decision from
GPL-3.0-or-later to AGPL-3.0-or-later and reworks the consequences, which have
changed materially since the first draft: ADR 0004 now puts a network server in
the project, which is exactly what AGPL's §13 is about.

## Context

The license has to be set before the first line of product code, because
changing it later requires the agreement of every copyright holder. Four things
constrain the choice:

- **What the customer wants from the project.** Answered: a free-software
  project in the Inkscape tradition, with the hosted-fork loophole closed. The
  only money is to come from asset-library providers (ADR 0007), not from the
  application, and not from plugins.
- **Code we might want to reuse.** The adjacent ecosystem is copyleft:
  Inkscape is GPL-2/3, Ink/Stitch is GPL-3, OpenVoronoi is LGPL. Reusing any
  GPL code requires a GPL-compatible license. ADR 0003 currently avoids all of
  it by going pure-Rust, so this preserves an option rather than meeting a
  present need.
- **There is now a server.** ADR 0004 adds a collaboration relay and cloud-sync
  service that users reach over a network. Under GPL-3.0 a third party could
  run a modified version of it as a closed service; under AGPL-3.0 they cannot.
  The customer's reason for choosing AGPL is precisely this.
- **Two dependency ecosystems.** ADR 0001 adds an npm frontend (React,
  Tailwind, shadcn/ui — all MIT) alongside the Rust tree (MIT/Apache-2.0).
  Neither imposes anything, but both now have to be checked. The previous
  coupling worth naming — ADR 0001's Slint option being GPLv3-or-paid — is
  gone with Slint.

### Options considered

**A. GPL-3.0-or-later.** Strong copyleft; matches the tools being replaced and
keeps every GPL codebase available for reuse. Does not stop a hosted fork of
the server or the browser build from staying closed. Rejected for that reason.

**B. AGPL-3.0-or-later.** Everything in A plus §13: anyone who lets users
interact with a modified version *over a network* must offer those users the
source of their modified version. This closes the loophole that matters now
that ADR 0004 has a server and ADR 0001 has a browser build. Costs: strictly
more hostile to any commercial add-on story, and some users, distributions and
organizations refuse AGPL software outright. **Chosen.**

**C. MPL-2.0.** File-level copyleft: our files stay open, the app can be
combined with proprietary modules. The best fit *if* paid plugins or a
proprietary edition mattered. They explicitly do not. Rejected.

**D. MIT OR Apache-2.0** (the Rust ecosystem default). Maximum adoption, zero
friction. Rejected: it gives away every protection for a project whose stated
motivation is that a commercial vendor abandoned the customer's platform.

### `-or-later`, decided separately

`AGPL-3.0-or-later` over `AGPL-3.0-only`. It lets the project adopt a future
AGPL version without collecting permission from contributors, it is what the
FSF recommends and what the adjacent projects do, and GPL-3.0 code stays
combinable either way (GPLv3 §13 permits combination with AGPLv3 code). The
cost is that recipients may use the work under a future version whose terms we
have not read; with the customer as sole copyright holder and a dual-license
option retained (§3), that is an acceptable, conventional risk.

## Decision

1. The project is licensed **AGPL-3.0-or-later**. `LICENSE` at the repo root,
   `license = "AGPL-3.0-or-later"` in `[workspace.package]`, the same SPDX
   identifier in the frontend `package.json`, and SPDX headers not required per
   file.
2. **Dependency licenses are allow-listed and checked in both ecosystems.**
   `cargo deny` carries an explicit allow-list (MIT, Apache-2.0, Apache-2.0
   WITH LLVM-exception, BSD-2/3-Clause, ISC, Unicode-3.0, Zlib, MPL-2.0,
   CC0-1.0). The npm tree gets the equivalent check in CI with the same list,
   since `cargo deny` cannot see it. Anything else fails CI and needs a
   decision, not a waiver. No AGPL-incompatible dependency (CDDL, SSPL, BUSL,
   or any "source-available" license) is accepted anywhere, **including in the
   server crate**, which is the part most likely to be offered such a
   dependency.
3. **Copyright stays with the customer.** Every contribution — including code
   written by agents in this repository — is attributed to the repository
   owner, so dual-licensing or relicensing later remains possible without
   chasing signatures. If outside contributors appear, a DCO or CLA is required
   *before* the first external PR is merged; after that the option is gone.
4. **Plugins are covered by this license; no linking exception is granted.** A
   plugin compiled against the ADR 0005 interface is treated as a derivative
   work and must be AGPL-compatible. This matches "there will be no paid
   plugins" and "everything else is deliberately free". It is permanent and
   commercially relevant, so it was put to the customer explicitly and
   confirmed; reversing it would need a new ADR and every copyright holder's
   agreement.
5. **Asset libraries, fonts, example files and material presets are not
   covered by this ADR.** Each needs its own license recorded next to it, and a
   font's own license governs redistribution regardless of the application's
   license. This is what keeps ADR 0007's monetization path open: data shipped
   or sold alongside is not a derivative work of the application.

This is a decision about project intent, not legal advice; if money becomes
involved the customer should confirm it with a lawyer.

## Consequences

- Anyone distributing a modified version must publish their changes, and
  **anyone running a modified collaboration or sync server for others must
  offer its source to those users** (§13). The "vendor drops Linux support and
  the users are stuck" failure mode cannot happen to this codebase, and neither
  can a closed hosted fork. That is the whole point of the change from A to B.
- **Our own hosted service must comply too, and that is cheap for us**: the
  server source is in this repository and published anyway, so §13 is satisfied
  by a visible source link in the service's UI. The obligation does become a
  real one if we ever run patches that are not in the public tree — so the
  policy is that we do not; the running service is built from the public tree.
- **The browser build is a §13 surface, not just a distribution.** Serving a
  modified browser build to users over a network triggers the same obligation
  for whoever serves it. Intended.
- Every GPL codebase in the adjacent ecosystem (Inkscape, Ink/Stitch) stays
  available for reuse — worth real money if embroidery stitch generation turns
  out harder than expected.
- **AGPL has real adoption costs**, and they are accepted, not dismissed: a
  number of companies forbid AGPL software outright, some distributions and app
  stores treat it awkwardly, and it will deter some casual contributors. For a
  tool whose users are makers and small shops rather than enterprises, this is
  the right trade — but it is a trade.
- **A proprietary third-party plugin ecosystem is foreclosed** by §4. Nothing
  about paid *asset libraries* is affected: a library is data, we neither host
  nor sell it, and ADR 0007 only stores a token and makes requests.
- Selling binaries, support or hosting of this application stays possible for
  the copyright holder; selling a *closed* derivative, or running one as a
  service, does not.
- shadcn/ui components are copied into our tree rather than depended on. Their
  MIT notices must be preserved in those files; the files themselves become
  part of the AGPL work. Routine, but it has to actually be done.
