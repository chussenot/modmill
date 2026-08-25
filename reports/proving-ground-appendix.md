# Appendix: post-mortem audit + testimony

Run against `reports/proving-ground.md` (the drafted prose) on modmill
`master` @ `9357c1f`, pact `0.15.1` (`6d36a62c925b`), recount `0.2.0`.
Read-only except bead creation and this file. Every number below is
pulled from `pact audit --json` / `recount testify` / `recount explain`
output saved under this run's scratchpad, or from `.pact/events.jsonl`
and `.beads/interactions.jsonl` directly with `jq` — not from the
drafted report's prose.

## Pass A — the machine record

### A1: gate-order reports exactly the planted violation, nothing else

```json
"gate_violations": [
  {"bead":"modmill-4r0.14","wave":5,"gate":"modmill-4r0.9","gate_wave":4,
   "agent":"w3-sequencing-rogue","via":"lease",
   "started_at":"2026-08-25T11:14:03.632526894+00:00",
   "gate_closed_at":"2026-08-25T11:25:20.415186411Z",
   "early_by_secs":676,"line":36},
  {"bead":"modmill-4r0.14", ... "early_by_secs":540,"line":37}
]
```

**Confirmed.** Exactly 2 events, exactly 1 bead (`modmill-4r0.14`), the
one planted violator — nothing else appears. `gate_closed_at` correctly
resolves to the **second** (re-close) `bd close` event on `modmill-4r0.9`
at `11:25:20Z`, not the first close at `11:09:02Z` — verified directly
against `.beads/interactions.jsonl`'s two `status→closed` rows for that
issue. This is pact's documented last-write-wins behavior working
correctly on a real double-close, not a bug.

### A2: the single expired event, and its classification

```json
{"path":"src/engine/effects/arpeggio.rs","agent":"w3-arpeggio",
 "opened_at":"2026-08-25T11:26:48...+00:00",
 "closed_at":"2026-08-25T12:42:54...+00:00","closed_by":"expired",
 "renewals":0,"held_secs":4566,"ttl_secs":2700}
```

**Confirmed**, agent/branch match the announced F1b kill target exactly
(`w3-arpeggio`, `worktree-agent-a9722882aea29f06f`). **One correction to
the drafted report's own language**: pact's `stale-holds` check and its
docs (`docs/audit.md`) contain no "active vs silent" field or
distinction anywhere — `grep -n "silent\|active" docs/audit.md` finds
`silent` used only for `--check silent-contention` (an unrelated check)
and for `handoff_coverage`'s `silent: [...]` list (beads that didn't
hand off — also unrelated to holds). The check is binary: a hold is
either in `stale_holds[]` or it isn't. "Liveness-aware stale-holds
split — active vs silent" was the run's own descriptive framing
(distinguishing the one `expired` hold from the 32 that ended by
ordinary `released`), not a pact-native classification. **Finding is on
the report side** — accurate in substance, imprecisely sourced. No bead
filed; noted here for the record.

### A3: handoff coverage

```json
"handoff_coverage": {
  "with_dependents": 14, "handed_off": 8,
  "silent": ["modmill-4r0.4","modmill-4r0.5","modmill-4r0.6",
             "modmill-4r0.7","modmill-4r0.8","modmill-4r0.9"]
}
```

**Confirmed exactly.** W1 (`.1`/`.2`/`.3`) has zero entries in `silent`
— 3/3 handed off, 100%. The 6 silent beads are precisely the parser
modules + both gates + engine-core, silent by design (no downstream
bead needed a finding from them) — named, not hidden, matching A3's
bar.

### A4: the 4 context events

```json
"context": {"commit-policy":"per-task","note":"0.15 proving ground",
            "scheduler":"waves-then-free-run",
            "topology-expectation":"worktrees"}
```

**Confirmed.** Exactly 4 keys, rendered in the summary header verbatim.

### The rest of the battery

`retry-storm`, `double-win`, `silent-contention`, `chain-integrity`,
`topology --expect worktrees --allow-main orchestrator`, and
`merge-divergence` all ran clean and matched their own documented
behavior on this ledger — no misclassification, no error, nothing
contradicting their own docs. **`claim-lease-divergence` could not run**
(`no beads data (no assignee history in .beads/interactions.jsonl)`) —
already understood precisely (not re-litigated here): `bd update
--claim` never emits an `assignee` field-change row, only a bare
`--assignee=` write does, despite `BD_AUDIT_ENABLED=1` being set the
entire run. Filed previously as `pact-6wb`; still open, still accurate.

**`commit-correlation` still finds real gaps, and one more than the
drafted report counted**: 12 uncovered commits now (was 11 when the
report was drafted), all orchestrator-authored, all missing a
covering lease. The twelfth is new since the report: a `README.md`
edit (commit `9357c1f`, adding the real-module collect/play section)
made without leasing `README.md` first — the same pattern recurring,
not a one-off. This directly motivates `pact-uwc` below.

## Pass B — testimony

### B1: THE ZERO

```
$ jq -c 'select(.harness_subagent != null)' .pact/events.jsonl | wc -l
0
$ jq -c 'keys' .pact/events.jsonl | sort -u   # harness_subagent never even appears as a key
$ grep -rl "PACT_HARNESS_SUBAGENT" .../subagents/*.jsonl   # zero files
```

0/74 events, confirmed two ways: the field never appears as a JSON key
(not merely null), and the string `PACT_HARNESS_SUBAGENT` appears in
**no** subagent transcript — not in the orchestrator's prompts to any
of the 15 subagents, not in any subagent's own commands. **Answer:
instruction absent from the brief** — the orchestrator (this run)
received the instruction in the original prompt ("agents export
PACT_HARNESS_SUBAGENT per the local recipe if one exists") and never
propagated it into any subagent prompt.

**But the deeper check changes what that fact is worth.** Reading
pact's own source settles whether there was anything valid to
propagate: `src/harness.rs`'s `harness_subagent()` doc comment states,
measured on real Claude Code behavior (2026-08-19), that a spawned
subagent's environment is identical to its parent's and its own
transcript id appears nowhere in it — "under Claude Code, nothing
[sets it], and the field is simply absent there" — and
`docs/fleet-patterns.md` §"PACT_HARNESS_SUBAGENT" says the same thing
to fleets directly: *"Set it only if your harness or spawner tells you
the id... the one pact can fingerprint today does not."* Both are
accurate and correctly hedged. **There was no recipe to propagate.** A
maximally diligent orchestrator following the brief's own "if one
exists" clause to the letter would have checked, found none, and set
nothing — exactly what happened, just without the checking step made
explicit. Net effect on outcome: zero. **No pact-repo bead filed for
B1 itself** — it correctly redirects to B2, where the real, fixable gap
lives.

### B2: pact-h8d, precisely characterized and relocated

`pact-h8d` was filed in the **pact** repo. It is entirely a **recount**
bug — filing it there was this run's own mistake, caught in this pass.
Closed with a pointer; refiled as **recount-n7q** with the exact
mechanism, found by reading `recount/src/join.rs` directly rather than
inferring from CLI output:

- `keyed_match()`'s `subagent: None` arm joins straight to
  `<session_dir>/<harness_session>.jsonl` — the top-level session's own
  file — and if it exists, asserts as fact: *"Whoever wrote this event
  owns that session and is nobody's subagent."* It never checks whether
  a `subagents/` directory with candidate `.jsonl` files (overlapping
  the query window) exists before asserting that.
- Confirmed on two independent recount commands against this ledger —
  `testify` (batch) and `explain --event-line 36` (single-event) —
  both returning `tier: "keyed", confident: true, lead: false` pointing
  at the orchestrator's top-level session, for events whose real
  authors were `subagents/agent-ae8938bfd605bf0d5.jsonl` (the gate-order
  violation) and `subagents/agent-a9722882aea29f06f.jsonl` (the
  stale-hold) respectively — both files sitting in the same session
  directory `keyed_match` already had open and stat'd, unexamined.
- **The delta from the report's manual F1b resolution**: the report
  found the right transcript by hand, using the event's own `worktree`
  field (`"worktree":"agent-a9722882aea29f06f"`) to construct the path
  `subagents/agent-a9722882aea29f06f.jsonl` directly. `keyed_match()`
  has that same `worktree` field available on the query and does not
  use it. That's the fix spec, not a vague "add a fallback."
- Pact's own design intent, quoted in `harness.rs`, says the opposite
  should happen: *"the keyed join tier will be rare, and the
  topological join stays the load-bearing path."* Measured here: keyed
  fired on **100%** of tested findings (3/3) and the topological ladder
  ran **0** times — "the topological ladder was not consulted" is
  recount's own evidence string, verbatim, on every one of them.

`recount-n7q` filed P1 in the recount repo, acceptance criteria written
against the exact code path, regression-test shape specified using
this run's real directory layout.

### B3: `recount explain` on three specific events

**The planted violation's claim** (`.pact/events.jsonl` line 36,
`recount explain --event-line 36`): same keyed/confident/wrong result
as B2, via a second command. The transcript that actually answers "did
the agent follow its brief" — pulled by hand from
`subagents/agent-ae8938bfd605bf0d5.jsonl` since recount couldn't reach
it — line 11:

> *"First lease acquired immediately at 2026-08-25T11:14:03.627236508Z
> (confirmed by 11:14:03.644 timestamp right after), with no waiting on
> G2. Now claiming the bead."*

This is the bad-agent-vs-bad-instruction excerpt the drafted report
already quoted; Pass B reconfirms it against the raw transcript file
and the exact ledger line together, rather than either alone.

**One renewal** (line 49, `w3-vibrato`): the same misresolution pattern
on `explain`. Pulled by hand from `subagents/agent-a65c3a35394cb0805.jsonl`,
**line 46** — stated *before* the renewal happened, as the agent's own
restated plan:

> *"Clean. Now claim the bead, then do the long-hold tuning pass across
> all 6 fixtures, renewing the lease partway through."*

The renewal itself lands at line 56–58, immediately after fixture 2/6
— exactly matching the orchestrator's prompt ("after renders 2-3 of 6,
run `pact lease renew`... required, not optional"). **Answer: renewed
from instruction, restated as the agent's own plan, not from
independent judgment.** The prompt asked for it explicitly; the
transcript shows compliance, not initiative. Worth naming plainly
rather than crediting the agent with judgment it didn't exercise.

**The vibrato bead's close, and whether inheritance could have
prevented modmill-89e** — yes, conclusively, by git timestamp:

```
docs/format-notes/effect-timing.md committed:  2026-08-25T12:50:26+02:00
  §6: "authentic PT does *not* reset the vibrato sine-table position
      on a plain new note ... A naive implementation that resets
      vibrato phase on every note will drift from authentic PT output
      and fail the byte-identical WAV conformance suite."
ch.vibrato_phase = 0 introduced:                2026-08-25T13:25:09+02:00
  (commit 43405676089a, "pre-wire tick-based dispatch for the 4
  remaining W3 effect beads" — ORCHESTRATOR-authored, not any bead)
```

The finding predicting this exact bug, in almost these exact words, was
committed **35 minutes before the bug was written** — by the
orchestrator, not by an agent who skipped its handoff. `w3-vibrato`
itself never had the chance to cause or prevent this: the reset was
already in shared `mod.rs` code it was told not to touch, and it
correctly flagged the divergence via handoff when it found it. **This
is a protocol learning, not a modmill or agent-compliance bug**: an
orchestrator pre-wiring shared skeleton code is bound by the same
"read your inheritance first" discipline as any claiming bead,
whenever that code encodes a behavioral default rather than pure
structural wiring — and there's no way to tell the two apart from
outside without reading the domain docs first. Filed as `pact-uwc`
(P2, `docs/fleet-patterns.md`) with this exact timeline and quote.

## Pass C — reconciliation table

| Report claim | Pass A/B measurement | Verdict | Side |
|---|---|---|---|
| Gate-order: exactly one violator | 2 events, 1 bead, confirmed via raw JSON | Match | — |
| "liveness-aware stale-holds split (active vs silent)" | No such field/term exists in pact; binary finding-or-not | Imprecise sourcing, substance correct | Report |
| Handoff coverage: W1 100%, gaps named | 3/3 W1, 6/6 named silent beads, confirmed via raw JSON | Match | — |
| 4 context events | Exactly 4 keys | Match | — |
| pact-h8d filed in pact repo | Bug is entirely recount's; wrong repo | Correction | Report (mis-homed) |
| pact-h8d description: "no fallback path...attributes to top-level session" | Precise mechanism: `keyed_match()`'s `subagent:None` arm asserts identity without checking for `subagents/` candidates; `worktree` field unused | Refined, not contradicted | Report (imprecise, now `recount-n7q`) |
| F1b kill/DEAD narrative | `stale_holds[0]` matches exactly: agent, branch, timestamps, `closed_by:"expired"` | Match | — |
| F1a renewal (fact of it) | Confirmed at line 49; transcript shows instruction-driven, not judgment | Match + new nuance | Report incomplete, not wrong |
| modmill-89e cause/timeline | Git-timestamp-verified: orchestrator wrote the bug 35 min after the doc that predicted it | New finding, not in original report | Appendix adds it |
| Commit-correlation: 11 uncovered commits | Now 12 — one more since the report, same pattern (README.md, unleased) | Report was correct as of its own snapshot; gap widened after | Both (time-bound) |

No divergence found where the drafted report was simply **wrong** about
a fact within its own snapshot — every difference above is either an
imprecise-but-substantively-correct phrasing, a mis-homed bead now
relocated, or new evidence gathered by this pass that the original
report didn't have (the vibrato timeline, the exact recount mechanism).

## Beads filed this pass

| id | repo | priority | subject |
|---|---|---|---|
| `recount-n7q` | recount | P1 | `keyed_match()` asserts "nobody's subagent" without checking for candidate subagent transcripts |
| `pact-uwc` | pact | P2 | fleet-patterns.md: orchestrator pre-wiring needs the same inheritance discipline as a bead |
| `pact-h8d` | pact | — | **closed**, relocated to `recount-n7q` |

(`pact-kum`, `pact-6wb` from the original report stand unchanged; not
re-litigated here since Pass A/B found nothing new about either.)

## Closing: which of pact's three features is field-proven

**Handoff is field-proven.** Every measurement in Pass A confirms it
exactly as designed: 100% W1 coverage, named gaps elsewhere, a
verbatim-quoted finding that changed downstream code, all verifiable
from the raw ledger with no interpretation required. Nothing about
this run's handoff evidence needed correction.

**Gate-order is field-proven as a detector, with its "honest reading"
promise also proven**: it caught exactly the one planted violation, no
false positives, no false negatives, chain-of-custody intact from
`plan.json` through the ledger. What Pass B adds is that the *human*
half of "the honest reading" — reading the violating agent's own
transcript to see whether it was following instructions — currently
requires bypassing recount entirely and reading `subagents/*.jsonl` by
hand, which worked, but is not the tool doing its job.

**Liveness is still mostly promise.** The mechanics work (TTL, expiry,
renewal are all correctly recorded and correctly computed against
their own recorded TTL, not a compiled-in constant). But the *readable*
half of liveness — `pact ui`'s `ACTIVE`/`IDLE`/`STALE`/`DEAD` roster,
and recount's ability to say *who* was behind any of those states —
depends entirely on the harness_subagent/topological-ladder machinery
that `recount-n7q` shows is currently broken for exactly the harness
shape (Claude Code + Agent-tool subagents) this run and presumably most
pact fleets actually use. The raw events are trustworthy; turning them
into "which agent, doing what, right now" without hand-grepping
transcripts is not, yet.

**The single highest-leverage fix**: `recount-n7q`. It's the one gap
that silently degrades a HIGH-confidence signal (`tier: keyed,
confident: true`) rather than an absent one — a consumer has no way to
know to distrust it — and it sits on the exact path pact's own design
already anticipated and explicitly delegated to recount
("the topological join stays the load-bearing path"). Fixing it
doesn't require any fleet to change how it runs, export anything new,
or adopt a workaround; it makes recount actually deliver the join pact
was already designed to hand off to it, for every fleet shaped like
this one — which, on the evidence of this run alone, is not a rare
shape.
