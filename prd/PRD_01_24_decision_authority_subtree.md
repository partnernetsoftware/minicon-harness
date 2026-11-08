# PRD 01.24 — decision authority subtree (T8)

Status: **study + structure only**; changes no source, version, capability
row, `cu-abi`, `[no]` row, G boundary or published asset. Owner of **who
decides what** in this repo. Learning ledger: `PRD_01_17`.

Owner direction (2026-11-06, 22:15):

> 为什么要我做决定，你能自己安排吗？
> ("Why am I making the decision? Can you arrange it yourself?")

This is an explicit correction, not a question to be answered with options.
The controller had ended the previous cycle by pasting the three G-boundary
choices back to the owner. That was an **escalation error**: a reversible,
high-confidence decision with an obvious safe default was pushed upward. Under
the standing `/goal` delegation the controller owns the whole loop — learn,
structure, plan, order versions, open gates, implement, release, accept — and
reports the decision with its reason. Nothing in this document asks the owner
to choose.

## T8 — decision authority tree

```text
T8 Decision authority (controller-autonomous under /goal)
├── D1 owner directions, recorded verbatim with date                [v]
│   └── /goal, 2026-11-06 day/evening, 2026-11-06 22:15 correction
├── D2 controller decisions — no prior owner approval              [decided]
│   ├── D2.1 version topic, candidate ordering, gate, W1-W4 timing
│   ├── D2.2 additive capability inside cu-abi 1.0 + 12/10/8/0 matrix
│   ├── D2.3 lifting one [no] with dated reason and explicit bounds
│   ├── D2.4 the G boundary direction (this document: (a))
│   └── D2.5 release chain, CI gates and owner-view acceptance
├── D3 owner veto, after the fact — never a pre-approval gate      [v]
│   └── one sentence from the owner overrides any D2 decision
├── D4 escalation discipline                                      [no]
│   ├── [no] pasting options + waiting for a reversible decision
│   │        with a safe default; that was the 22:15 correction
│   └── unlock: only an irreversible choice with no defensible default
│        (deleting published assets, changing the product definition),
│        where the controller must still record the reason and stop
└── D5 decision record format                                     [v]
    └── date / choice / reason / evidence grade / reopen trigger
```

`[decision]` in every other document means **a dated decision already taken
by the controller**, not "waiting for the owner". Historical records that say
"owner decision" are kept verbatim where the owner actually spoke (for
example the 0.0.11 a11y lift); from 2026-11-06 22:15 the same class of
decision is D2 and is taken by the controller.

## Decision record 2026-11-06 22:15 — the G boundary

**Choice: (a) keep G closed.** `screen` stays K/V/M/W/S plus the window
target; process, CPU, memory, GPU, device and network state remain outside
it. The whole resource domain continues to be learned and structured in
`PRD_01_21` as `[study]`, with no authority attached.

**Reason.** `PRD_01_08` makes G a structural boundary, not a missing verb.
Direction (b) (typed read-only telemetry in `disclose`) still widens what
`screen` may observe and needs a threat model, three-OS probes and an
independent reader; direction (c) needs a fourth tool or an ABI change, new
grants, a policy engine and an audit surface — a different product from a
bounded one-file worker. With the current phase being STUDY + STRUCTURE,
opening either is not defensible. Keeping G closed is reversible and has a
zero-risk default, so it is decided, not offered as a choice.

**Reopen triggers for (b), all four required, judged by the controller:**

1. X-grade evidence from three-OS read-only probes, with named `unknown`
   reasons on every cell that cannot report a value.
2. A dedicated PRD with its own threat model, evidence semantics and the
   exact additive rows (no new verb, no new grant, no `cu-abi` bump).
3. An independent reader rule plus a three-OS CI journey that fails closed.
4. A plan-completeness pass in `PRD_01_23`.

**Direction (c) is `[no]`** unless the product definition itself changes;
that is D4 territory, not a version decision.

**Evidence grade:** O (`PRD_01_08` boundary), X (this repo's shipped
12/10/8/0 matrix and four-backend gate). No new capability is claimed.

## Decision record 2026-11-06 — stop W2 after the W1 probe

**Choice:** stop 0.0.15 W2 (UIA-first + MSAA fallback), claim no UIA
capability, run no version bump and no Release chain. Keep
`examples/cu_probe_uia.rs` in the tree as reproducible X-grade evidence.
`v0.0.14` remains the last shipped Release.

**Reason.** The W1 probe on the real desktop returned `S_OK` from every COM
slot and a usable `RECT` from the desktop root, while Brave / Sourcetree / GVIM
/ cmd produced hundreds of named, bounded, non-Pane rows. But both WorkBuddy
windows (the L3 self-hosted-orchestration target) exposed only 8 nodes, all
Pane, only one named and **zero non-Pane controls** — the shipped 0.0.11 MSAA
tree reads real controls on that same client. Cold-start reads also returned
all-zero rectangles with `S_OK` before the same handle settled to
`840,0,1680,1014` on four consecutive re-runs. Shipping UIA-first would
therefore make `observe.a11y` *worse* on the exact window the end state needs,
and the plan's mandatory downgrade rule (written before W1, not invented
afterwards) requires stopping on the zero-rectangle reads. No owner choice was
requested.

**Reopen trigger (sharpened by W0 pass 6):** a probe that reads named,
non-Pane, stable bounded nodes **from a WorkBuddy client whose full
accessibility mode is enabled**, across repeated reads, plus its own PRD and
independent reader rule. Pass 6 found that Chromium builds its full web a11y
tree lazily (`--force-renderer-accessibility`, an assistive-technology
presence, or Electron's `setAccessibilitySupportEnabled`); enabling that in
the *target app* is outside `screen` and changes neither this repo's grant
nor the G boundary. A successful `CoCreateInstance` on some other window, or
a single non-empty tree, is not sufficient.

**Evidence grade:** X (local measurement, `plan-0.0.15` W1 delivery record).

## Decision record 2026-11-06 22:15 — working cadence

The controller arranges the rest itself, in this order, without returning for
a choice:

1. W0 pass 4: verify the UIA COM surface against the **locally installed
   Windows SDK headers** (grade O/X), replacing the second-hand vtable claim;
   learn the resource-telemetry trust boundary as a study note. **Done.**
2. Upsert T1/T3/T5/T7/T8, the W0 ledger and the 0.0.15 plan in place. **Done.**
3. Complete the 0.0.15 plan-completeness checklist (6/6) and mark W1
   eligible. **Done.**
4. Start W1 (Rust UIA diagnostic probe) after step 3, then follow its
   downgrade rule. **Done — the rule fired, W2 is stopped (decision above).**
5. Continue W0 with the next open-gate candidate; report decisions, reasons
   and results, not a menu.

## Decision record 2026-11-07 — switch the 0.0.15 route

**Choice:** re-scope 0.0.15 from "UIA probe verdict, W2 stopped, no version
bump" to **a11y-addressed actuation**: `act:control` gains `source` and
`role`, with a semantic MSAA-node arm. The UIA-first W2 stays `[no]`; the
diagnostic plan and the pass-5 probe remain verbatim in the tree. W1-W4 run
after a new 6/6 plan (`plan-0.0.15-a11y-addressed-actuation.md`) is complete.

**Reason.** W0 pass 7 produced X-grade evidence from this repo's own CU wire
on the real L3 target: `observe.a11y` reads 43 MSAA nodes, 8 buttons and 11
named controls with 42 non-zero rectangles, while `observe.controls` is
refused on that self-drawn window. The pass-5 "zero addressable controls"
finding was **UIA-provider-specific** (Chromium builds its web a11y tree
lazily); MSAA on the same hwnd is populated. The desktop was therefore
observable but not addressable — a real gap in computer use, and the one the
owner's end state needs. The new route is additive inside `cu-abi 1.0` and the
12/10/8/0 matrix: no new verb, grant, crate, platform library, ABI bump or
matrix cell, and no `[no]` beyond the already-stopped UIA W2 is lifted. The
plan's downgrade rule was written before W1. Keeping G closed is unchanged.

**Evidence grade:** X (local measurement, W0 pass 7), O/S (2026 grounding
literature, study only). A study note did not authorise the switch; this dated
controller decision did, under D2.1 (version topic) and D2.3 (lifting a
scoped `[no]` with explicit bounds). No owner choice was requested or pasted;
the owner keeps the post-hoc veto (D3).

**Reopen trigger for the UIA W2:** unchanged from the 2026-11-06 record — a
probe reading named, non-Pane, stable bounded nodes from a WorkBuddy client
with full accessibility mode enabled, across repeated reads, plus its own PRD
and independent reader rule. This decision does **not** satisfy that trigger.

## Learning notes (dated)

| Date | Pass | What was queried | Source class | Conflict / caution recorded | Went to |
|---|---|---|---|---|---|
| 2026-11-06 | 4 | local `UIAutomationClient.h` / `.idl` vtable, CLSID/IID, `RECT`, `TreeScope` | O (installed SDK 10.0.22000.0) | a header is authoritative for the SDK it ships with, not for every runtime; a probe must still print raw HRESULTs | T1 P2.4, `plan-0.0.15` |
| 2026-11-06 | 4 | host-side vs guest-side resource telemetry trust | O / P / S | guest self-reports are debugging data, never a security boundary; host-side signals sit on the far side of the isolation boundary | T3 R9/R10 |
| 2026-11-06 | 4 | decision authority under `/goal` | owner direction | a reversible default must not be escalated; the owner keeps a veto, not an approval queue | this file, T8 |
| 2026-11-06 | 5 | W1 UIA probe verdict on the real desktop | X (this machine) | every slot S_OK + usable desktop root RECT, yet the target provider gave 8 Pane nodes / 0 non-Pane controls; cold-start zero RECTs became stable on re-run. The downgrade rule stopped W2 without an owner choice | `plan-0.0.15`, `PRD_01_17` pass 5, T7 |
| 2026-11-06 | 6 | lazy Chromium `AXMode`, Linux/macOS a11y grant tiers | O / S / V | the empty tree is a target-app launch precondition outside `screen`; this sharpens the W2 reopen trigger but starts no implementation | `PRD_01_17` pass 6, this file |
| 2026-11-07 | 7 | semantic grounding, native default action, MSAA/UIA provider split, real-target a11y vs Win32 scan | O / P / S / X | a native press has no pointer readback; "no addressable controls" was UIA-specific; MSAA on the same hwnd yields 8 buttons. Study does not authorise a route, a dated decision does | `PRD_01_17` pass 7, `PRD_01_25`, this file |
| 2026-11-07 | 8 | expected-effect-first verification, CU TOCTOU, asymmetric a11y event coverage, resource-boundary reading, real-desktop before/after probe | O / S / V / X | the target window has no editable `accValue`, so a "changed" success would invite fabrication; event subscription would tear the three-OS matrix. The v1 0.0.16 contract was **falsified by measurement**, not by preference | `PRD_01_17` pass 8, `plan-0.0.16` v2, this file |
| 2026-11-07 | owner | line freeze: CU effort through 0.0.18; omp (oh-my-Pi) harness reference only after 0.0.18 acceptance | owner direction | "roughly after" is an unlock window, not a version commitment; 0.0.17/0.0.18 topics stay decided by gate + fresh evidence | `PRD.md` roadmap, `PRD_01_23` C9 |

## Dated controller decision D6 (2026-11-07, W0 pass 8)

**Choice:** keep 0.0.16's topic (F3 first half, a11y effect readback) but
**upgrade its contract** from "publish a `changed` array" to "compare a
pre-declared `expect` against a recomputed `observed`", with `unchanged` /
`mismatch` as honest verdicts; keep event subscription and the harness branch
`[no]`; start no code, test, version bump or Release.

**Reason:** the X-grade probe measured 43 identity-stable nodes with zero
before/after differences, a single non-empty `value` (a document URL), and a
performed hover with no state change. Requiring a non-empty `changed` would
make the target window permanently fail or reward a fabricated difference.
Pass 8's expected-effect-first and TOCTOU findings independently require a
pre-declared expectation and a generation-bound `before` frame. The v1
assumption was written before the measurement; a measurement outranks a plan.

**Evidence:** `PRD_01_17` pass 8 table + `plan-0.0.16` v2 "X-grade evidence";
`src/cu.rs` generation/frame lifecycle read during planning.

**Trigger to revisit:** a WorkBuddy provider that publishes an editable
textbox `accValue` on the real desktop, or a three-OS event surface with
symmetric coverage; either would reopen the fixture choice, not the G
boundary. The harness line reopens only on the owner's 0.0.18 condition.

## Dated controller decision D7 (2026-11-07, W2 fixture)

**Choice:** the three-OS journey verifies effect evidence on the existing
0.0.15 side-effect-free hover — `expect: []` → `unchanged`,
`expect: ["value"]` → `mismatch`. `confirmed` and `vanished` are verified by
injected-frame unit tests instead of a required CI step that types into
Notepad.

**Reason:** 0.0.15 already hit a Notepad dirty-marker race (`*Untitled -
Notepad`) that made a required `act_type` step report `unknown` on one CI run.
A second typed act would put that race inside the 0.0.16 acceptance path. The
reader still recomputes `observed`, `expect`, `state_names`, both generations
and the verdict from the published rows, and rejects a non-refused row on an
empty tree — **no assertion is weakened**. The plan's Windows `confirmed`
fixture remains a documented option with a reopen trigger, not a claim.

**Evidence:** local journey + independent reader (only the pre-existing
maximised-window `act_move` refusal fails), real-desktop X-grade probe gen
1→2 / 2→3, 133 unit tests and the four-configuration type gate.

**Trigger to revisit:** a CI cell that reliably exposes an editable a11y
textbox whose `accValue` changes on a typed act, with an independent reader
assertion for `confirmed` that weakens no earlier step.

## Dated controller decision D8 (2026-11-07, APE size ceiling)

**Choice:** keep the 6 MiB Candidate ceiling of `PRD_01_01` unchanged. The
four APE cells with no platform a11y provider short-circuit `effect` to the
named `no_a11y_tree` refusal; the pure comparison stays platform-independent
and unit-tested on all three OSes, but is dead code in those four cells and
fat LTO drops it. New schema/description strings were shortened, and the
refusal row dropped fields that no reader recomputes.

**Reason:** the 0.0.16 pack measured 6,317,398 bytes — 25,942 over the
ceiling. `PRD_01_01` marks `[no] auto-raising the size ceiling` and requires
an explicit, dated **owner** decision; raising it is not the controller's
call. All six APE cells carry a copy of the Rust code, and four of them can
never publish an a11y tree, so the comparison path is genuinely dead there.
The zip overlay was already deflate level 9 and the release profile already
`opt-level=z / fat LTO / strip / panic=abort`, so no honest size knob remained.

**Evidence:** 133 unit tests + four-configuration type gate green, real-desktop
X-grade probe, local release build with no warnings; three-OS journey
refusals remain named and reader-admissible.

**Trigger to revisit:** a non-Windows cell that gains an a11y provider, or a
future version whose unavoidable growth cannot be recovered by dead-code
elimination. Either reopens the ceiling question **to the owner**, never as an
automatic bump.

**Outcome (2026-11-07 20:03):** the revisit fired as designed. A third pack
after a final string trim measured 6,307,626 bytes — still 16,170 over with no
honest knob left — and the controller presented the blocked pack with three
priced options and stopped. The owner answered by re-sending the standing
`/goal` with "完全放权给你自主推进"; under that full delegation the ceiling
was raised to the minimal 6,553,600 bytes (6.25 MiB, 4.2% headroom over the
measured pack) and recorded as a dated decision in `PRD_01_01`. This D8
record and the failed pack runs 42368000472 / 42369268313 / 42371125571 are
the evidence chain; no automatic bump happened at any point.

## Dated controller decision D9 (2026-11-08, post-0.0.18 line choice)

**Choice:** with `v0.0.18` published and accepted, the owner's line-freeze
condition is satisfied and the harness/omp reference is eligible. The
controller defers starting harness work to 0.0.19's planning cycle and will
decide then, on the evidence: first a W0 study pass over omp's harness
approach (previously forbidden, now permitted), weighed against the CU
candidate pool (subset-expectation matching, F3 second half, incremental
events). No harness study, code or dependency starts before that dated
decision.

**Reason:** the owner said the omp reference is expected "roughly after
0.0.18" — an eligibility window, not a mandate. The CU branch still has
ungated, L3-critical candidates, and choosing blind between the two lines
without a W0 pass would be the same mistake as committing 0.0.17/0.0.18
topics in advance.

**Evidence:** `v0.0.18` acceptance record in `plan-0.0.18`; candidate pool in
`PRD_01_17` / `PRD_01_23`.

**Trigger to revisit:** the 0.0.19 planning cycle itself.

## Non-goals

- `[no]` re-opening the G boundary in this phase.
- `[no]` adding a resource-domain verb, tool, grant or capability row.
- `[no]` treating a study note as a decision, or a decision as evidence.
- `[no]` rewriting published version history or shipped Release assets.
