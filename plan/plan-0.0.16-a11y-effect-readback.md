# 0.0.16 — a11y effect readback (planned, NOT started)

Status: **planned after `v0.0.15` was published and accepted from the owner's
view (2026-11-07)**. This is the *next version plan*, written down before any
code, test, version bump or Release. All four version sources stay `0.0.15`.
Owner direction (`/goal`): plan and implement dynamically; after each public
Release, plan the next version; concentrate on computer use; use
minicon-harness's own CU, never Python; fill in whatever is missing. Owner
direction (2026-11-06 evening): do not start code or tests until the plan is
complete — 三思后行 / 分而治之. This file is that completion, not a start.

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
observe(a11y: hwnd) -> a11y_effect row
```

`A11yControl.value` (`accValue`), `state`, `name`, `role_name`, `rect`,
`depth` and `child_id` are already published with the 0.0.14 normalised
schema, and 0.0.13 already provides the bounded wait that lets a Chromium
reflow settle. No new native API, OS grant, image arm, model loop, crate or
permission surface is needed. This is additive evidence, not a new verb.

## Exact contract (additive; `cu-abi 1.0` unchanged)

- `observe` keeps `a11y`, `wait_ms`, `controls`, `capture` exactly as shipped.
- A new optional `effect` parameter on `observe`: an object naming one a11y
  node identity (`hwnd`, `name`, optional `role`, `child_id`) from the **same
  host's previous frame**. The host compares the previous frame's node with
  the fresh scan and publishes `evidence["a11y_effect"]`:
  ```text
  { "before": {value, state, state_names, rect, depth, child_id},
    "after":  {value, state, state_names, rect, depth, child_id} | null,
    "name", "role_name", "hwnd",
    "changed": ["value" | "state" | "rect" | "name"] (typed, bounded),
    "verdict": "changed" | "unchanged" | "vanished" }
  ```
- Identity is matched by `name` + `role_name` + `child_id` + `depth`, never
  by a coordinate. A node that disappeared is `vanished`, not `changed`.
- `state_names` is derived from the raw `state` bit mask by the same fixed
  table as 0.0.14; the independent reader recomputes it. No guessed names.
- `changed` is a sorted whitelist computed from the two published row objects;
  the reader recomputes the set and rejects a guessed or unsorted one.
- An `effect` request with no previous frame, a stale generation, no matching
  node, or a refused a11y scan is a **named refusal** with an empty row —
  never a fallback to pixels, never a plausible `changed`.
- `press` still defaults to false everywhere; nothing in this version clicks
  a button merely because an effect was requested.

## Downgrade rule (mandatory, written before W1)

Stop and publish the diagnosis instead of shipping if: the same node cannot
be identified across two generations; `value`/`state` can flip without a
generation change; a refused scan could still publish a `changed` row; an
ambiguous identity resolves to an arbitrary node; or any 0.0.6-0.0.15 reader
assertion has to be weakened. No success may be reported from the request.

## Reader contract (independent, `scripts/cu-journey-verify.py`)

- Every 0.0.6-0.0.15 assertion stays, unchanged and in the same order,
  including the 0.0.15 `act_control_a11y` recomputation.
- New `observe_a11y_effect` step: recompute `state_names` from both raw
  `state` masks, recompute `changed` from the two published row objects,
  reject a negative/zero rect, reject `changed` containing a non-whitelist
  value, reject a `performed` row whose verdict disagrees with the recomputed
  difference, and reject any effect row on a cell whose a11y tree is empty.
- A named `no_frame` / `stale` / `no_a11y_tree` / `no matching node` refusal
  is admissible and is never counted as `[v]`.

## Three-OS behaviour

| Cell | Behaviour |
|---|---|
| Windows | MSAA tree is real; a typed act changes the target's `accValue`, so `a11y_effect` publishes a recomputable `changed: ["value"]` |
| macOS / Linux / fallback | `a11y_controls: []` with the existing named reason; `effect` is a named `no_a11y_tree` refusal, never a guess |
| matrix | 12/10/8/0 unchanged; no new grant, crate, link-time platform library, native signature, verb or capability row |
| four-cfg type gate | `cu-cfg-check.py` types all four configurations in the same commit |

## Non-goals

- `[no]` a model-visible image arm (A5/D2), batched `actions[]` (B8), a model
  loop, an iteration/cost ceiling policy, session management or result
  aggregation. Those keep their own PRDs and unlock conditions.
- `[no]` AT-SPI / AX trees, UIA routing, native default-action dispatch
  (`accDoDefaultAction` / `AXUIElementPerformAction` / `atspi.DoAction`),
  incremental a11y event subscription or any control-pattern invoke.
- `[no]` app-level *semantic* claims ("the sub-task finished"): this version
  only proves that a published node attribute changed. L3 aggregation is not
  claimed.
- `[no]` widening `screen` into host resources; `PRD_01_08` G stays closed,
  direction (a) decided in `PRD_01_24`.
- `[no]` re-publishing or overwriting any shipped v0.0.4-v0.0.15 asset.
- `[no]` treating this plan as a version commitment or as evidence.

## Plan-completeness checklist (`PRD_01_23`) — 6/6, plan only

| # | Requirement | Where | State |
|---|---|---|---|
| 1 | exact contract: `effect` identity, typed row, whitelist | "Exact contract" | `[v]` plan |
| 2 | downgrade branch: stale, vanished, ambiguity, refused scan | "Downgrade rule" | `[v]` plan |
| 3 | reader contract: additive rows, retained assertions, recomputation | "Reader contract" | `[v]` plan |
| 4 | three-OS behaviour and four-cfg gate | "Three-OS behaviour" | `[v]` plan |
| 5 | non-goals and G boundary restated | "Non-goals" | `[v]` plan |
| 6 | W1 eligible only after 1-5, with a dated controller decision | this table | `[v]` plan |

**A plan fact is not a capability.** W1/W2 may start only after a dated T8 D2
decision, and only three-OS CI plus owner-view acceptance can produce `[v]`.
The ordering rationale is in `PRD_01_17`; the gates for L2/L3 are in
`PRD_01_12` / `PRD_01_23`. The controller decides the topic itself and
reports the reason — no owner choice is requested.

## Work breakdown (for the next cycle, not started)

```text
0.0.16 — a11y effect readback
├── W1 Rust: pure before/after node diff + `effect` admission + unit tests
├── W2 journey + independent reader + four-cfg type gate + three-OS CI
├── W3 four version sources + README/PRD truth
└── W4 public signed Release + owner-view acceptance + next-version plan
```
