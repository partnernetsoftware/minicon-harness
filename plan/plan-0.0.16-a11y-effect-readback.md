# 0.0.16 — a11y effect readback (planned v2, NOT started)

Status: **planned after `v0.0.15` was published and accepted from the owner's
view (2026-11-07)**; **upserted to v2 on 2026-11-07 after W0 pass 8 and a
real-desktop X-grade probe**. This is the *next version plan*, written down
before any code, test, version bump or Release. All four version sources stay
`0.0.15`. Owner direction (`/goal`): plan and implement dynamically; after
each public Release, plan the next version; concentrate on computer use; use
minicon-harness's own CU, never Python; fill in whatever is missing. Owner
direction (2026-11-07): **effort stays on the computer-use branch**; the
harness branch is not to be improved by reference to omp (oh-my-Pi) until
roughly *after* `v0.0.18`. Owner direction (2026-11-06 evening): do not start
code or tests until the plan is complete — 三思后行 / 分而治之. This file is
that completion, not a start.

## Why this topic (the gap 0.0.15 leaves)

`PRD_01_12` states the L3 requirement precisely: *"an actuation receipt alone
is not enough here, because it proves a button was posted, not that a sub-task
started or produced output"*. 0.0.15 made the L3 client addressable by
identity (a named MSAA button can be resolved and hovered/pressed), but the
evidence chain still ends at a pointer readback inside a rectangle. W0 pass 7
(`PRD_01_17`) records the 2026 consensus **"verify after every step"** and
that the dominant long-horizon failure is skipped verification, not a missing
click primitive.

The bounded first half of F3 is therefore **a typed readback of the same a11y
node before and after an act**, using only abilities already shipped:

```text
observe(a11y: hwnd) -> act:type / act:control -> observe(wait_ms) ->
observe(a11y: hwnd, effect) -> a11y_effect row
```

`A11yControl.value` (`accValue`), `state`, `name`, `role_name`, `rect`,
`depth` and `child_id` are already published with the 0.0.14 normalised
schema, and 0.0.13 already provides the bounded wait that lets a Chromium
reflow settle. No new native API, OS grant, image arm, model loop, crate or
permission surface is needed. This is additive evidence, not a new verb.

## W0 pass 8: three findings that change the design (2026-11-07)

1. **Expected effect must be written before the act.** A 2026 system-design
   treatment of CU verification (`vibeengines.com/ai-system-design/computer-use-agent-system-design`)
   and the 2026 GSoC OpenVINO CU write-up both make the same point: showing a
   new frame to a model afterwards invites post-hoc rationalisation; the
   decision must **emit its predicted effect as part of the action**, so
   verification is a *comparison*, and a mismatch is a countable event. The
   OpenVINO stack lists control read-back, controls-diff and focused-control
   read-back as ground truth, with a perceptual hash only as a no-op detector.
   → 0.0.16 is **expect-vs-observed**, not "did anything change".
2. **TOCTOU: an observation cannot authorise a later act.** AIDEFEND's 2026
   computer-use TOCTOU case (a Continue button replaced by Send at the same
   coordinates during inference) makes pre-dispatch re-validation mandatory:
   bind the planned effect to element identity and re-check it at dispatch;
   record before / action-time / post-action evidence. → the `before` frame
   is bound to its **generation**, and any intervening observation is a
   named stale refusal.
3. **Poll, do not subscribe (yet).** MSAA exposes `EVENT_OBJECT_VALUECHANGE`
   / `EVENT_OBJECT_STATECHANGE` (Microsoft Event Constants), and a 2026
   cross-platform a11y event reference (`xa11y.dev/reference/events`) shows
   the coverage is asymmetric: Linux delivers all state bits, Windows only
   IsEnabled / ToggleState / ExpandCollapse / window visual state, macOS only
   Checked / Busy / Minimized; `MenuOpened`/`MenuClosed` are unreliable on
   Linux and `WindowActivated` absent on Windows. Subscribing would tear the
   12/10/8/0 matrix and add an OS grant surface. → 0.0.16 diffs two
   **published snapshots**; incremental event subscription stays `[no]`.
   Resource-domain reading (cgroup v2 is a boundary, not a placement policy;
   MIG is hardware partitioning; `CUDA_VISIBLE_DEVICES` alone is not an
   isolation boundary; PSI-driven protection precedes OOM) is a T3 study note
   only, and does not lift G.

## X-grade evidence gathered during planning (no code, no version bump)

Measured with the shipped CU wire (`examples/cu_orchestrate.rs`, `op=observe`
/ `op=act`) on the real desktop, hwnd `33689578` (WorkBuddy self-drawn
window). All analysis was done on the published JSON rows, never on pixels
and never with Python touching the desktop.

| Measurement | Result | Consequence |
|---|---|---|
| two consecutive `observe(a11y)` | 43 / 43 rows; identity set (`name`+`role_name`+`child_id`+`depth`) **equal**; state / value / rect differences **0 / 0 / 0** | cross-generation identity matching is viable; the tree is stable enough to diff |
| nodes with non-empty `value` | **1**, a `document` node whose value is the `file:///...` renderer URL | the v1 assumption "a typed act changes the target's `accValue`" is **false on this window**; there is no editable textbox value in this snapshot |
| `act:control` hover on `最小化` (`press=false`) | `Performed`, node resolved by identity; before/after state / value / rect differences **0 / 0 / 0** | a no-op act must be reported honestly as `unchanged`, not dressed up as `changed` |
| Windows CI a11y scan (0.0.15 journey) | Notepad `Text Editor` textbox node exists and is selectable | this is the side-effect-free `expect:["value"]` → `confirmed` fixture for the three-OS reader |

**Design correction (v2):** the v1 contract implied success meant a
non-empty `changed` array. On the target window that would either always
fail or, worse, invite a fabricated difference. v2 makes `unchanged` a
first-class verdict — detecting a silent no-op is the capability — and adds
`expect`, written before the act, so `confirmed` / `mismatch` are
recomputable comparisons. The v1 text is not preserved as authority; this
plan is the current one, and the correction is dated in `PRD_01_24`.

## Exact contract (additive; `cu-abi 1.0` unchanged)

- `observe` keeps `a11y`, `wait_ms`, `controls`, `capture` exactly as shipped.
- A new optional `effect` object on `observe`:
  ```text
  { "hwnd": u64, "name": string, "role": string?, "child_id": i32?,
    "expect": ["value"|"state"|"rect"|"name"] }
  ```
  `expect` is a **bounded, sorted, de-duplicated whitelist** and must be
  present in the request; it is the predicted effect written before the act.
  An empty `expect` means "predict no change" and is legal (it can only ever
  produce `unchanged` or `mismatch`).
- The host publishes `evidence["a11y_effect"]`:
  ```text
  { "before": {value, state, state_names, rect, depth, child_id},
    "after":  {value, state, state_names, rect, depth, child_id} | null,
    "name", "role_name", "hwnd",
    "expect": [...], "observed": [...],
    "verdict": "confirmed" | "mismatch" | "unchanged" | "vanished" }
  ```
  where `observed` is the sorted whitelist of fields that actually differ
  (the v1 `changed` field, renamed to make the comparison explicit), and:
  `confirmed` = `observed` is non-empty and equals `expect`; `unchanged` =
  `observed` is empty and `expect` is empty; `mismatch` = they differ;
  `vanished` = the node no longer exists (`after: null`).
- **Frame binding (verified against `src/cu.rs`):** every successful
  `observe` increments `state.generation` and overwrites `state.frame`;
  `act` does not increment it. Therefore the effect must be computed inside
  the `observe` handler, **reading the old `state.frame` as `before` before
  overwriting it**, with the fresh snapshot as `after`. `before.generation`
  must equal the generation at dispatch time; the new frame is that value
  +1. `effect` itself adds no extra generation bump. This is the TOCTOU
  re-validation point; a generation gap is a named `stale` refusal.
- Identity is matched by `name` + `role_name` + `child_id` + `depth`, never
  by a coordinate. A node that disappeared is `vanished`, not `mismatch`.
- `state_names` is derived from the raw `state` bit mask by the same fixed
  table as 0.0.14; the independent reader recomputes it. No guessed names.
- An `effect` request with no previous frame, a stale generation, an empty
  or refused a11y scan, no matching node, an ambiguous identity, or an
  invalid `expect` is a **named refusal** with an empty row — never a
  fallback to pixels, never a plausible `observed`.
- `press` still defaults to false everywhere; nothing in this version clicks
  a button merely because an effect was requested.

## Downgrade rule (mandatory, written before W1)

Stop and publish the diagnosis instead of shipping if: the same node cannot
be identified across two generations; `value`/`state` can flip without a
generation change; a refused or stale scan could still publish a non-empty
`observed`; `expect` can be supplied or altered *after* the result is known;
`confirmed` can be produced without a matching `expect`; an ambiguous
identity resolves to an arbitrary node; or any 0.0.6-0.0.15 reader assertion
has to be weakened. **No success may be reported from the request.** A
no-op act reported as `unchanged` is a valid result, never a failure to be
hidden.

## Reader contract (independent, `scripts/cu-journey-verify.py`)

- Every 0.0.6-0.0.15 assertion stays, unchanged and in the same order,
  including the 0.0.15 `act_control_a11y` recomputation.
- New `observe_a11y_effect` step: recompute `state_names` from both raw
  `state` masks, recompute `observed` from the two published row objects,
  recompute `verdict` from `expect` vs `observed`, reject a non-whitelist /
  unsorted / duplicated `expect`, reject a negative-or-zero rect, reject
  `confirmed` without equality, reject a `performed` row whose verdict
  disagrees with the recomputation, and reject any effect row on a cell whose
  a11y tree is empty.
- A named `no_frame` / `stale` / `no_a11y_tree` / `no matching node` refusal
  is admissible and is never counted as `[v]`.

## Three-OS behaviour (corrected by the X-grade measurement)

| Cell | Behaviour |
|---|---|
| Windows CI journey | an editable `Text Editor` textbox node is selected; `expect:["value"]` is written before `act:type`; the effect row recomputes `observed:["value"]` → `confirmed` |
| Windows real desktop (planning probe) | identity-stable 43-node tree; a no-op hover with empty `expect` → `unchanged`; a `value` expectation on the WorkBuddy window → `mismatch`, both recomputable. No fabricated `changed` |
| macOS / Linux / fallback | `a11y_controls: []` with the existing named reason; `effect` is a named `no_a11y_tree` refusal, never a guess |
| matrix | 12/10/8/0 unchanged; no new grant, crate, link-time platform library, native signature, verb, event subscription or capability row |
| four-cfg type gate | `cu-cfg-check.py` types all four configurations in the same commit |

## Non-goals

- `[no]` improving the **harness branch** in this cycle, and `[no]`
  researching/implementing an omp (oh-my-Pi) harness reference before
  `v0.0.18` is published and accepted. Owner direction 2026-11-07; the
  earliest eligibility is *after* that acceptance, and the topic is still
  decided by the controller then, not promised by this plan.
- `[no]` a model-visible image arm (A5/D2), batched `actions[]` (B8), a model
  loop, an iteration/cost ceiling policy, session management or result
  aggregation. Those keep their own PRDs and unlock conditions.
- `[no]` AT-SPI / AX trees, UIA routing, native default-action dispatch
  (`accDoDefaultAction` / `AXUIElementPerformAction` / `atspi.DoAction`),
  incremental a11y event subscription (pass 8 finding 3) or any
  control-pattern invoke.
- `[no]` app-level *semantic* claims ("the sub-task finished"): this version
  only proves whether published node attributes matched a pre-declared
  expectation. L3 aggregation is not claimed.
- `[no]` widening `screen` into host resources; `PRD_01_08` G stays closed,
  direction (a) decided in `PRD_01_24`. Resource-domain pass-8 reading is a
  T3 study note only.
- `[no]` re-publishing or overwriting any shipped v0.0.4-v0.0.15 asset.
- `[no]` treating this plan as a version commitment or as evidence.

## Plan-completeness checklist (`PRD_01_23`) — 6/6, plan v2 only

| # | Requirement | Where | State |
|---|---|---|---|
| 1 | exact contract: `effect` identity, `expect`, typed rows, frame/generation binding | "Exact contract" | `[v]` plan |
| 2 | downgrade branch: stale, vanished, ambiguity, refused scan, post-hoc `expect` | "Downgrade rule" | `[v]` plan |
| 3 | reader contract: additive rows, retained assertions, recomputation | "Reader contract" | `[v]` plan |
| 4 | three-OS behaviour and four-cfg gate, corrected by X-grade measurement | "Three-OS behaviour" | `[v]` plan |
| 5 | non-goals, G boundary, and the harness-line freeze restated | "Non-goals" | `[v]` plan |
| 6 | W1 eligible only after 1-5, with a dated controller decision | this table, `PRD_01_24` | `[v]` plan |

**A plan fact is not a capability.** W1/W2 may start only after a dated T8 D2
decision, and only three-OS CI plus owner-view acceptance can produce `[v]`.
The ordering rationale is in `PRD_01_17`; the gates for L2/L3 are in
`PRD_01_12` / `PRD_01_23`. The controller decides the topic itself and
reports the reason — no owner choice is requested.

## Work breakdown (for the next cycle, not started)

```text
0.0.16 — a11y effect readback (F3 first half)
├── W1 Rust: pure before/after node diff + expect comparison + `effect`
│            admission (old frame read before overwrite) + unit tests
├── W2 journey + independent reader + four-cfg type gate + three-OS CI
├── W3 four version sources + README/PRD truth
└── W4 public signed Release + owner-view acceptance + next-version plan
```

## Roadmap boundary (owner direction 2026-11-07)

```text
v0.0.16  CU: a11y effect readback                [_] this plan, NOT started
v0.0.17  CU: chosen from the candidate pool by gate + fresh X-grade evidence
v0.0.18  CU: chosen likewise; still no harness-line work
> v0.0.18 (after public acceptance)  harness branch MAY be improved with
          omp (oh-my-Pi) as a reference — deferred, not a commitment
```

0.0.17 / 0.0.18 are **not** topic commitments: the W0 frontier radar upserts
candidates with unlock conditions. What is decided here is the *line*: the
computer-use branch owns versions up to and including 0.0.18, and the harness
branch stays frozen until after that Release is accepted from the owner's view.
