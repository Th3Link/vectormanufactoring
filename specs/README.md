# Feature specs

Spec-driven development, adapted from
[spec-driven-dev-kit](https://github.com/trojava/spec-driven-dev-kit) for
this project's roles and file layout (see `CLAUDE.md` §2, §4).

One folder per feature: `specs/<NNNN-feature-slug>/`, three files. `NNNN`
is a 4-digit, zero-padded sequence number matching the feature's position in
`specs/index.md`'s `#` column — the same numbering convention
`docs/adr/NNNN-slug.md` already uses for ADRs. Assign the next number when a
feature's folder is created; numbers are never reused or renumbered later.

## `specification.md` — what and why (product-owner)

```text
# <feature title>

Status: Draft | Ready | In progress | Done
Priority: Must | Should | Could
Origin: Customer | Proposal

## User value
As a <maker role> I want <capability> so that <outcome>.

## Acceptance criteria
1. Given ..., when ..., then ...

## Out of scope
- ...

## UX notes
(filled in by ux-engineer before Ready)

## Links
Requirements: ...  PR: ...
```

## `adrs.md` — which decisions apply (architect)

```text
# ADRs for <feature title>

- [ADR 000X](../../docs/adr/000X-slug.md): <one line on what it decides
  for this feature>

## Feature-local decisions
Anything too small for a full ADR, dated:

- YYYY-MM-DD: <decision> — <why>
```

## `plan.md` — how it gets built (implementer)

```text
# Plan for <feature title>

## Affected crates/modules
- ...

## Tasks
- [ ] 1. <task> (fulfils AC 1, 2)
- [ ] 2. <task> (fulfils AC 3)

## Validation
How this gets verified beyond the tester's acceptance tests (e.g. golden
files, property tests, manual check with hardware).
```

## Lifecycle

Same stages as `CLAUDE.md` §4:

1. **Ready** — `specification.md` + `adrs.md` exist, UX notes attached if
   the feature has UI.
2. **Build** — implementer writes `plan.md`, then works on branch
   `story/<feature-slug>` in a worktree, checking off tasks as it goes.
3. **Verify** — tester works from `specification.md` only; `ux-engineer`
   and `architect` review per `CLAUDE.md` §4.
4. **Demo** / **Done** — as in `CLAUDE.md` §4; `specification.md` gets
   `Status: Done` and the PR link.

A feature folder's `specification.md` is the single source of truth for
what "done" means. `plan.md` tracks progress; it is not a second backlog.
