# 0.0.18 — a11y text write with expect-vs-observed verification (planned, NOT started)

Delivery record (W1-W3, 2026-11-08; W4 pending):

- **W1 probe** (`examples/cu_probe_write.rs`): on the real WorkBuddy client,
  the search panel's `搜索对话内容` textbox accepted `put_accValue` (slot 27,
  S_OK). Two decisive findings: (a) the write is **asynchronous on Chromium**
  — an immediate same-COM-session readback still showed the old value, while
  a fresh `observe(a11y)` through the shipped wire read `value="cu-0018"`;
  (b) the write's collateral effects (rect reflow, tree diff) belong in the
  caller's `expect` set. `SysAllocStringLen`/`SysFreeString` live in
  oleaut32, not ole32.
- **W2**: `act:control` gains `set_value` (a11y arm only, exclusive with
  `press` and `invoke`); readonly (0x40) and protected (0x10) state vetoes
  fire before any native call; the write is dispatched by identity through
  the 0.0.17 walker; evidence publishes `set_value`, `hresult`, the resolved
  node, no pointer actuation, and an explicit `effect_hint` naming the
  `expect:["value"]` verification contract. Journey steps
  `act_control_set_value` / `observe_a11y_effect_set_value` with reader
  recomputation (a named `skipped` is admissible when the cell's first
  interactive node is not a textbox). Real-desktop X-grade loops:
  `expect:["value"]` → `mismatch observed:["rect","tree","value"]` (honest —
  the reflow is real) and the full-set `expect:["rect","tree","value"]` →
  `confirmed`. 138 unit tests, four-cfg gate green.
- **W3**: four version sources → `0.0.18`. W4 not claimed until it runs.

Status: **planned after `v0.0.17` was published and accepted from the owner's
view (2026-11-08)**. All four version sources stay `0.0.17`. Owner directions
in force: `/goal` full delegation; concentrate on computer use; never Python
for CU; **harness/omp reference only after `v0.0.18`** — this is the last CU
version before that line reopens; plan 6/6 before code (三思后行 / 分而治之).

## Why this topic (the gate analysis)

The candidate pool's remaining rows are all gated on things this repo decided
not to open: per-window capture router needs the A5/D2 image-arm budget,
incremental a11y events need three-OS AT-SPI/AX grants, resource telemetry is
behind the closed G boundary (a) with four reopen triggers, and omp is frozen
until after this very version. The one capability gap that is (i) ungated,
(ii) pure CU, and (iii) squarely on the L3 path (`PRD_01_12`: the self-hosted
client must "start a sub-task" — which begins with typing into a field) is
**semantic text write**: the 0.0.15-0.0.17 chain can locate, hover, click and
invoke a node by identity, and verify node/tree effects — but it still cannot
*set a value*. MSAA's own `IAccessible::put_accValue` (vtable slot 25's
neighbour in the property family) is the same hand-written-FFI shape as
everything already shipped, and its effect is directly verifiable with the
0.0.16 `expect:["value"]` readback — no new evidence kind is needed.

## Scope (additive; `cu-abi 1.0` and 12/10/8/0 unchanged)

- `act:control` with `source=a11y` gains an optional **`set_value`** string —
  the only new surface. Mutually exclusive with `press` and `invoke`.
- Windows backend: `IAccessible::put_accValue(child, BSTR)` dispatched to the
  object located by full identity, exactly like 0.0.17's invoke. No new
  crate, COM library, OS grant or window activation.
- Refusals: non-text nodes, read-only state bit (`STATE_SYSTEM_READONLY`
  0x40), protected fields, and every 0.0.17 admission rule unchanged.
- Evidence: `set_value`, the raw `hresult`, the resolved node, no pointer
  actuation; the caller verifies with `observe(effect)` and
  `expect:["value"]` (already shipped in 0.0.16 — the Notepad `Text Editor`
  textbox is the three-OS journey fixture that was designed for exactly this).
- macOS / Linux / fallback: the same named unsupported boundary as the invoke.

## W1 probe (before any shipped capability)

Extend the `cu_probe_invoke.rs` precedent: a diagnostic that locates an
editable textbox (the WorkBuddy search input once opened via the 0.0.17
invoke, or Notepad), calls `put_accValue`, and reads the value back through
the shipped wire. Gates W2: if no editable field accepts a programmatic write
(Electron often refuses `put_accValue` on internal fields), the downgrade rule
fires and this version stops with the diagnosis published — 0.0.18 would then
ship a smaller additive topic chosen from the study notes instead.

## Downgrade rule (mandatory, written before W1)

Stop and publish instead of shipping if: `put_accValue` returns success but
the published value never changes on a known-editable field; the write steals
focus or triggers a native dialog; a protected (password) field can be
written; or any 0.0.6-0.0.17 reader assertion must be weakened.

## Reader contract

All 0.0.6-0.0.17 assertions unchanged. New step `act_control_set_value`: the
reader recomputes the resolved node, requires a declared `expect:["value"]`
on the following effect step, and recomputes the value delta. A named
`unsupported` boundary on non-Windows cells and a named read-only refusal are
admissible.

## Non-goals

`[no]` AT-SPI/AX writes, UIA ValuePattern, batched actions, event
subscription, image arm, model loop, G-boundary change, harness-line work
(this is the last version before the omp reference becomes eligible, and even
then the controller decides the topic then), and no commitment for the
post-0.0.18 version.

## Plan-completeness checklist — 6/6 (plan only, not a capability)

| # | Requirement | Where | State |
|---|---|---|---|
| 1 | exact contract: `set_value`, admission, evidence, refusal set | Scope | `[v]` plan |
| 2 | downgrade branch defined before W1 | Downgrade rule | `[v]` plan |
| 3 | reader contract additive | Reader contract | `[v]` plan |
| 4 | three-OS behaviour: Windows real, others named boundary | Scope | `[v]` plan |
| 5 | non-goals + line-freeze boundary restated | Non-goals | `[v]` plan |
| 6 | W1 probe-first; start only on dated T8 decision | W1 probe | `[v]` plan |

## Work breakdown (not started)

```text
0.0.18 — a11y text write + effect verification
├── W0 pass 10  put_accValue semantics + editable-field probe design
├── W1 probe    real-desktop write on an editable field; gate W2
├── W2 impl     set_value admission + FFI + unit tests + journey + reader
├── W3 bump     four version sources + docs truth
└── W4 ship     three-OS CI + signed Release + owner-view acceptance
```
