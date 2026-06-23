---
status: active
created: YYYY-MM-DD
updated: YYYY-MM-DD
adrs: [NNNN]
---

# Plan: <short work-item title>

## Intent

<What this work-item delivers and why, in 2–5 sentences: the problem and the
outcome. No solution detail here — that is Approach.>

## Approach

<How, at a glance. Link the ADR(s) that decided it —
Per [ADR-NNNN](../../adr/NNNN-title.md) — and note related ADRs/plans in prose.
Mark anything still undecided with `[NEEDS CLARIFICATION: <question>]`; none
may remain before the task it gates is started.>

## Tasks

<A checklist of stable-id items. Ids (`T1`, `T2`, …) are assigned on creation
and **never renumbered** — a split or reordered task gets a *fresh* id, and an
existing id never silently changes meaning. `current-task.md` points at these
ids. Check an item off only with a one-line **Done:** as-built note.>

- [x] **T1** — <task> **Done:** <one-line as-built note; SHAs aggregate in `commit refs:` at archive>
- [ ] **T2** — <task not yet started>

## Decision log

<Small decisions local to this work-item that don't warrant an ADR, plus the
as-built reality pulled out of the implementing ADR (the ADR records the
decision; the plan records how it actually landed). When in doubt between a new
ADR and an entry here, prefer here.>

- <decision> — <why>

<!-- ON COMPLETION — move this file to docs/plans/archive/<slug>.md, and in the
     SAME commit repoint the implementing ADR's `**Implemented by:**` line to
     that archive path (appending the implementing commits). Amend the
     frontmatter above — keep `created` unchanged, add/update these keys:

       status: done
       created: <unchanged>
       updated: <date>
       completed: <date>
       adrs: [NNNN]
       commit refs: [<the implementing commits — NOT the archival move>]

     Every task must be [x] with a **Done:** note before archiving. -->
