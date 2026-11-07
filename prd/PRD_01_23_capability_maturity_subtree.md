# PRD 01.23 — capability maturity and version subtree (T5 + T7)

Status: **study + structure only** (owner direction 2026-11-06 evening:
learn and structure first; **do not start code or tests until the plan is
complete**). Changes no source, version, capability row, `cu-abi`, `[no]`
row or the G boundary. Sibling subtrees: `PRD_01_19` (T1), `PRD_01_20` (T2),
`PRD_01_21` (T3), `PRD_01_22` (T4/T6). Learning ledger: `PRD_01_17`.

Legend: `[v]` shipped with three-OS evidence, `[_]` candidate / planned,
`[no]` explicit non-goal, `[study]` research note.

## T5 — capability and maturity tree

```text
T5 Capability (A-F index) and maturity (L0-L3)
├── A Observation             -> PRD_01_19 (T1)  [v] A1-A4, [_] A5
├── B Actuation               -> PRD_01_20 (T2)  [v] B1-B7b, [_] B8
├── C Contract and safety     -> PRD_01_20 (T2)  [v] C1-C4, [_] C5/C6, [no] C7
├── D Loop and state          -> PRD_01_19 (T1)  [v] D1/D4-partial, [_] D2/D3
├── E Platform and env        -> PRD_01_22 (T4)  [v] E1-E5, [no] Wayland-native
├── F Verification/delivery   -> PRD_01_22 (T6)  [v] F1/F2, [_] F3
├── L0 read-only observation                     [v]
│   └── evidence: A1-A4, C1-C4, E1-E5, F1/F2
├── L1 evidenced interaction                     [v]
│   └── one effect per call with a desktop readback; coordinates supplied
├── L2 self-sustained loop                       [_] not shipped
│   ├── gate 1: A5/D2 model-visible image arm                    [no] open
│   ├── gate 2: B7b + D4 bounded wait and ceilings               [v] 0.0.13
│   ├── gate 3: A4 / equivalent reliable target on three OSes    [_] open
│   └── gate 4: F3 app-level effect evidence                     [_] open
└── L3 self-hosted orchestration                 [_] target, not shipped
    ├── L2 + F3 + C5/C6 safety and human hand-off
    └── session management / result aggregation in the client GUI
```

The A-F tree is an **index of engineering concerns**, not a definition of the
domain; `PRD_01_08` owns the K/V/M/W/S domain and the G boundary. L0-L3
describe evidence and closure, never a new permission surface. The threshold
the owner asked about is **L2**; L3 is the trustworthy aggregation form.

## T7 — version and candidate state tree

```text
T7 Versions / candidates (state, not a promise)
├── V0.0.1 - V0.0.5   foundation: 3 tools, 6 cells, signing court      [v]
├── V0.0.6   button actuation receipt + verify_actuation                 [v]
├── V0.0.7   drag path[] + two-axis scroll evidence                      [v]
├── V0.0.8   held-key modifiers + teardown release                       [v]
├── V0.0.9   environment / grants[] disclosure                           [v]
├── V0.0.10  display scale / coordinate basis                            [v]
├── V0.0.11  Windows MSAA structured control tree                        [v]
├── V0.0.12  handle-addressed focus + target_hwnd                        [v]
├── V0.0.13  bounded observation wait + wait budget                      [v]
├── V0.0.14  normalised a11y node schema                                 [v]
├── V0.0.15  a11y-addressed actuation — public signed Release           [v]
│   ├── W0 pass 5: UIA diagnostic probe (X-grade, kept as evidence)    [v]
│   ├── W0 pass 7: real MSAA target measurement + route switch         [v]
│   ├── W1/W2: `source`/`role`, resolver, reader, four-cfg gate        [v]
│   └── W3/W4: four version sources + public signed Release            [v]
├── V0.0.16  a11y effect readback — expect-vs-observed        [_] W4 pending
│   ├── plan v2 6/6 complete 2026-11-07 (W0 pass 8 + X-grade probe)  [v] plan
│   ├── W1/W2/W3 complete: 133 tests, four-cfg gate, X-grade probe,
│   │   two three-OS journey steps + independent reader,
│   │   four version sources -> 0.0.16                                [v] local
│   └── W4 three-OS CI + public signed Release + owner-view acceptance [_] pending
├── Candidate pool (ordered by gate, not by version)                     [study]
│   ├── C1 UIA mixed routing              gate: **closed by pass 5 probe** [no]
│   ├── C2 AT-SPI / AX trees              gate: OS grants + 3-OS fixture [_]
│   ├── C3 incremental a11y events        gate: schema + 3-OS stream     [_]
│   ├── C4 per-window capture router      gate: A5/D2 image budget       [_]
│   ├── C5 F3 app-level effect evidence   gate: side-effect-free fixture [_]
│   ├── C6 C5/C6 safety + hand-off       gate: policy PRD first          [_]
│   ├── C7 B8 batched actions            gate: admission/stale design    [_]
│   ├── C8 resource-domain telemetry     gate: 4 triggers (T8), G closed  [_] deferred
│   └── C9 harness-line improvement     gate: v0.0.18 published+accepted   [no] until then
└── Standing [no] (unchanged by any study pass)                          [no]
    ├── plugin host, permission policy engine, streaming, MCP
    ├── a scripting surface over the desktop, a second model loop
    ├── takeover / owner-lock / watch mode, any new crate or runtime
    ├── runtime backend selection, Wayland-native protocol
    └── host-resource management (G; PRD_01_08 owns the boundary)
```

## Current phase on 2026-11-06 evening (historical record, kept verbatim)

```text
STUDY  ->  STRUCTURE  ->  PLAN COMPLETION  ->  ONLY THEN CODE + TESTS
  ^            ^                ^
  |            |                |
  W0 passes    T1-T8 trees     0.0.15 plan completeness checklist
  + notes      + subtrees      (plan/plan-0.0.15-uia-probe-and-routing.md)
```

No W1/W2 code, no test, no version bump and no Release chain is started for
0.0.15. The last shipped version stays `v0.0.14`; all four version sources
read `0.0.14`. "三思后行" (think three times before acting) and "分而治之"
(divide and conquer) are the governing discipline of this phase: a large
branch gets its own subtree document and an explicit unlock condition before
any implementation is considered. W0 pass 4 closed the plan-completeness
checklist (6/6) by replacing the second-hand vtable claim with the installed
Windows SDK headers. W1 then ran on the real desktop and its downgrade rule
stopped W2, so no version bump or Release chain started. Decision authority is
T8 (`PRD_01_24`): the controller arranges the cadence itself.

## Current phase (owner direction 2026-11-06 evening; closed 2026-11-07)

```text
STUDY  ->  STRUCTURE  ->  PLAN COMPLETION  ->  ONLY THEN CODE + TESTS
  ^            ^                ^
  |            |                |
  W0 passes    T1-T9 trees     two 0.0.15 plan completeness checklists
  + notes      + subtrees      (UIA diagnostic, then the shipped route)
```

The STUDY + STRUCTURE phase is **closed**: W0 pass 7 measured an addressable
MSAA tree on the real L3 target, `plan/plan-0.0.15-a11y-addressed-actuation.md`
passed its own 6/6 checklist **before** any code started, and W1-W4 ran. The
2026-11-06 UIA probe record is preserved verbatim as history; the route
switch is a dated controller decision (`PRD_01_24`). "三思后行" and
"分而治之" still govern: T9 got its own subtree and an explicit unlock
condition before implementation. Decision authority is T8.

## Current phase (2026-11-07, after v0.0.15 acceptance)

```text
PLAN 0.0.16 v2 (6/6)  ->  dated T8 D6 decision  ->  ONLY THEN W1/W2
```

W0 pass 8 plus the real-desktop before/after X-grade probe corrected the
0.0.16 contract to **expect-vs-observed** (pre-declared `expect`, recomputed
`observed`, `unchanged` / `mismatch` as first-class verdicts, generation-bound
`before` frame, snapshot diff instead of event subscription). No code, test,
version bump or Release has started; all four version sources read `0.0.15`.
The CU branch owns 0.0.16-0.0.18 — topics for 0.0.17 / 0.0.18 are **not**
committed — and C9 (harness line / omp reference) is `[no]` until `v0.0.18`
is published and accepted. The historical phase sections above are kept
verbatim.

## Plan-completeness checklist for 0.0.15 — 6/6 passed 2026-11-06 (UIA probe)

W0 pass 4 verified the COM surface against the installed Windows SDK headers
(`PRD_01_17`, grade O) and the plan now writes down all six items
(`plan/plan-0.0.15-uia-probe-and-routing.md`). This is a **plan** fact only:
no source, test, version bump or Release asset changed.

1. `[v]` Exact COM surface: `CLSID_CUIAutomation`, three IIDs, the exact
   `IUIAutomation` / `IUIAutomationElement` / `IUIAutomationElementArray`
   vtable slots, `UIA_HWND`, `TreeScope_Subtree`, the `RECT` -> `[x,y,w,h]`
   conversion and the raw-HRESULT pass/fail rule.
2. `[v]` Downgrade branch: failed HRESULT, hung provider, unenumerable
   array, zero rectangle indistinguishable from a read failure, missing
   pattern — each stops with the failing slot named.
3. `[v]` Reader contract: additive `a11y_source` / `a11y_reason`, the 0.0.14
   schema kept intact, `state_names` recomputed, negative width/height
   rejected, every 0.0.6-0.0.14 assertion retained.
4. `[v]` Three-OS behaviour: Windows probes and falls back to MSAA with a
   named reason; macOS / Linux / fallback keep empty trees and named reasons;
   the four-backend type gate types all four configurations.
5. `[v]` Non-goals and the G boundary restated; G (a) is a decided
   controller decision (`PRD_01_24`), no `[no]` is lifted.
6. `[v]` W1 became eligible after 1-5, ran on 2026-11-06, and its verdict
   closed C1. W2, W3 and W4 are `[no]` for this cycle — the downgrade rule
   fired on the target WorkBuddy provider's empty rectangles.

## Plan-completeness checklist for 0.0.15 (shipped route) — 6/6 passed 2026-11-07

`plan/plan-0.0.15-a11y-addressed-actuation.md` was written and closed **before
W1 started**, after W0 pass 7's X-grade measurement. All six items: exact
`source`/`role` contract and resolution order (1), the downgrade branch for an
empty tree / zero rect / ambiguity / bad readback (2), the additive reader
contract with every earlier assertion retained (3), three-OS behaviour plus
the four-cfg gate (4), non-goals and the G boundary (5), and W1 eligibility
only after 1-5 (6). W1/W2 then produced 126 passing unit tests, a real-desktop
semantic hover on the `最小化` button, four green backend configurations and
an independent reader recomputing centre, containment and `state_names`.

## Learning notes (dated, process not just conclusions)

| Date | Pass | What was queried | Source class | Conflict / caution recorded | Went to |
|---|---|---|---|---|---|
| 2026-11-06 | 1 | L0-L3 framing and self-hosted orchestration | X (this repo) + S | L2 needs A5/D2 + B7b/D4 + A4 + F3, not one feature | `PRD_01_12`, T5 |
| 2026-11-06 | 2 | UIA gate downgrade to "probe required" | O / S | a published vtable order is not a verified slot on this machine | `PRD_01_17`, C1 |
| 2026-11-06 | 3 | candidate ordering by unlock condition | X + S | ordering is a reason to consider first, never a version commitment | T7 |
| 2026-11-06 | 3 | owner instruction to slow down before code | owner direction | earlier "implement next" delegation is superseded for this phase | this file, current phase |
| 2026-11-06 | 4 | installed Windows SDK COM headers, resource-telemetry trust, decision authority | O (installed SDK) / P / S / owner direction | a header proves slots for its SDK, not runtime behaviour; `RECT` != `[x,y,w,h]`; a reversible default was wrongly escalated | `PRD_01_17` pass 4, `PRD_01_24`, this checklist |
| 2026-11-06 | 5 | W1 probe verdict on the real desktop | X (this machine) | all slots S_OK and the desktop root read a usable RECT, but the target provider withheld all bounds; the downgrade rule stopped W2 | `plan-0.0.15`, T7 |
| 2026-11-06 | 6 | why the Electron tree was empty; Linux/macOS a11y gates | O / S / V | lazy `AXMode` is a target-app precondition outside `screen`; AT-SPI2 needs a session bus and macOS needs two TCC grants. Study only, no implementation | `PRD_01_17` pass 6, T7 |

## Non-goals

- `[no]` declaring any new `[v]`, capability row, ABI bump or matrix change
  from a study pass.
- `[no]` starting 0.0.15 W1/W2 code or tests before the checklist above is
  complete.
- `[no]` widening `screen` into host resources; C8 is deferred, G direction
  (a) is decided by the controller (`PRD_01_24`), not pending.
