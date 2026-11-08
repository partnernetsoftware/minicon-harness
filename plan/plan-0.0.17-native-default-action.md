# 0.0.17 — native default action with effect verification (planned, NOT started)

Delivery record (W1-W3, 2026-11-08; W4 pending):

- **W1 probe** (`examples/cu_probe_invoke.rs`): three real-desktop findings.
  (a) `窗口置顶` invoke returned S_OK but the *window minimized* — the
  dispatch-child default actions are real but not precisely targeted, so an
  invoke must never be assumed safe-by-name. (b) `对话内搜索` invoke really
  opened the search panel: the tree grew 43 → 48 nodes with new named nodes
  (`搜索对话内容`, `关闭搜索（Esc）`) — the effect is observable, but in the
  **tree diff**, not in the target node's own state/value. (c) invoking a
  child_id on the window object is a silent S_OK no-op on Electron; the live
  object must be located by full identity in the tree.
- **W2**: `act:control` gains `invoke:true` (a11y arm only, mutually exclusive
  with `press`); the native side walks the published tree by identity
  (name+role_name+child_id, first unique match) and invokes
  `accDoDefaultAction` **on the located object**; evidence publishes `invoke`,
  `hresult`, the resolved node and no actuation receipt (no pointer moved).
  The 0.0.16 effect whitelist gains the **`tree`** dimension: observed now
  includes a recomputable identity-set diff of the two frames' trees, and the
  row publishes `tree.before_nodes/after_nodes/changed`. 136 unit tests,
  four-cfg gate, journey steps (`observe_a11y_pre_invoke`, `act_control_invoke`,
  `observe_a11y_effect_invoke`) with the independent reader recomputing the
  tree dimension, hresult evidence and wait accounting. Real-desktop X-grade:
  invoke `对话内搜索` → S_OK → tree 43→48 → `confirmed observed:["tree"]` →
  Esc restore performed.
- **W3**: four version sources → `0.0.17`. W4 (three-OS CI, signed Release,
  owner-view acceptance) is not claimed until it has run.

Status: **planned after `v0.0.16` was published and accepted from the owner's
view (2026-11-07)**. This is the *next version plan*, written before any code,
test, version bump or Release; all four version sources stay `0.0.16`. Owner
directions in force: `/goal` full delegation; concentrate on computer use;
never Python for CU; harness/omp reference only after `v0.0.18`; plan 6/6
before code (三思后行 / 分而治之).

## Why this topic now (the gate actually opened)

The candidate table has carried this row since pass 7: *native default action
(`accDoDefaultAction` / `AXPress` / `atspi.DoAction`) — press by identity
without a pointer* with unlock condition **"three-OS probes + a new
effect-evidence kind + a policy PRD"**. 0.0.16 shipped exactly that evidence
kind: `evidence["a11y_effect"]` compares a **pre-declared `expect`** against a
recomputed `observed` from two published frames. A native invoke has no
pointer readback — the old reason it was `[study]` — but it *does* have an
a11y-state/value readback, which 0.0.16 made a first-class verdict. The gate
opened; nothing else in the pool has an open gate (AT-SPI/AX need OS grants,
capture router needs the A5/D2 image budget, batched actions need their own
admission design, omp is frozen until after 0.0.18).

L3 payoff: today pressing a WorkBuddy button needs a pointer landing inside a
rectangle on the foreground window. `accDoDefaultAction` invokes the *same*
node by identity with no pointer and no foreground steal, then 0.0.16's
`expect:["state"]` proves the press happened — the exact "press by identity,
verify by readback" loop the L3 self-hosting PRD (`PRD_01_12`) requires.

## Scope (additive; `cu-abi 1.0` and 12/10/8/0 unchanged)

- `act:control` with `source=a11y` gains an optional **`invoke: true`** — the
  only new surface. `invoke` is mutually exclusive with `press` (a pointer
  click); both default to false. Admission order: identity resolution and
  state vetoes exactly as 0.0.15, then the invoke.
- Windows backend: `IAccessible::accDoDefaultAction(child)` via the existing
  MSAA FFI surface. No new crate, COM library, OS grant or window
  activation; the call addresses the already-resolved node.
- macOS / Linux / fallback: named capability boundary (`unsupported`), exactly
  like `act:control` today. No AT-SPI/AX work in this version.
- Evidence: the invoke receipt publishes the resolved node, the raw HRESULT,
  and — required — the caller's `expect` for the following
  `observe(effect)` readback. An invoke without a declared expectation is
  refused (0.0.16 discipline: the expectation is written before the act).
- `press`-equivalent safety: `invoke` on a node whose default action is
  destructive is not special-cased here; the journey only invokes
  side-effect-free targets, and the same `[no]` irreversible-action gate as
  `press` applies.

## W1 probe (before any shipped capability)

A Rust diagnostic probe (`examples/`), following the `cu_probe_uia.rs`
precedent: enumerate the MSAA nodes of a real window, call
`accDoDefaultAction` on one **harmless** node (e.g., a checkbox in a test
window, or the WorkBuddy `对话内搜索` toggle re-toggled back), and read back
state before/after through the shipped wire. Verdict gates W2: if no node's
state/value changes measurably after an invoke, the downgrade rule fires and
this version stops with the diagnosis published.

## Downgrade rule (mandatory, written before W1)

Stop and publish instead of shipping if: `accDoDefaultAction` returns success
but the published node state/value never changes on a known-toggle target
(invoke would be unauditable); the call steals focus or activates the window
(breaks the background premise); the effect readback cannot distinguish an
invoke from a no-op (`unchanged` on every invoke); or any 0.0.6-0.0.16 reader
assertion must be weakened.

## Reader contract

All 0.0.6-0.0.16 assertions unchanged. New step `act_control_invoke`: the
independent reader recomputes the resolved node, requires the declared
`expect` in the request, and recomputes the following effect row's verdict
from the two published frames. A named `unsupported` boundary on non-Windows
cells is admissible.

## Non-goals

`[no]` AT-SPI/AX invoke, UIA `Invoke` pattern, batched actions, event
subscription, image arm, model loop, G-boundary change, harness-line work
(frozen until after `v0.0.18`), and any version/topic commitment for 0.0.18.

## Plan-completeness checklist — 6/6 (plan only, not a capability)

| # | Requirement | Where | State |
|---|---|---|---|
| 1 | exact contract: `invoke`, admission order, evidence, expect requirement | Scope | `[v]` plan |
| 2 | downgrade branch defined before W1 | Downgrade rule | `[v]` plan |
| 3 | reader contract additive | Reader contract | `[v]` plan |
| 4 | three-OS behaviour: Windows real, others named boundary | Scope | `[v]` plan |
| 5 | non-goals + line freeze restated | Non-goals | `[v]` plan |
| 6 | W1 probe-first; start only on dated T8 decision | W1 probe | `[v]` plan |

## Work breakdown (not started)

```text
0.0.17 — native default action + effect verification
├── W0 pass 9  accDoDefaultAction semantics + a harmless-target probe design
├── W1 probe   real-desktop invoke on a harmless node; gate W2
├── W2 impl    invoke admission + FFI + effect tie-in + unit tests + journey
├── W3 bump    four version sources + docs truth
└── W4 ship    three-OS CI + signed Release + owner-view acceptance
```
