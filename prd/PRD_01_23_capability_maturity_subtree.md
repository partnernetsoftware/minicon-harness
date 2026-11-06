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
├── V0.0.15  UIA probe-first / mixed routing  **planning, not started**  [_]
│   ├── W1 Rust UIA diagnostic probe (raw HRESULTs)                      [_]
│   ├── W2 UIA-first + MSAA fallback, only if W1 passes                  [_]
│   ├── W3 docs truth                                                     [_]
│   └── W4 version bump + public signed Release                          [_]
├── Candidate pool (ordered by gate, not by version)                     [study]
│   ├── C1 UIA mixed routing              gate: real-desktop probe       [_] 
│   ├── C2 AT-SPI / AX trees              gate: OS grants + 3-OS fixture [_]
│   ├── C3 incremental a11y events        gate: schema + 3-OS stream     [_]
│   ├── C4 per-window capture router      gate: A5/D2 image budget       [_]
│   ├── C5 F3 app-level effect evidence   gate: side-effect-free fixture [_]
│   ├── C6 C5/C6 safety + hand-off       gate: policy PRD first          [_]
│   ├── C7 B8 batched actions            gate: admission/stale design    [_]
│   └── C8 resource-domain telemetry     gate: explicit G decision       [decision]
└── Standing [no] (unchanged by any study pass)                          [no]
    ├── plugin host, permission policy engine, streaming, MCP
    ├── a scripting surface over the desktop, a second model loop
    ├── takeover / owner-lock / watch mode, any new crate or runtime
    ├── runtime backend selection, Wayland-native protocol
    └── host-resource management (G; PRD_01_08 owns the boundary)
```

## Current phase (owner direction 2026-11-06 evening)

```text
STUDY  ->  STRUCTURE  ->  PLAN COMPLETION  ->  ONLY THEN CODE + TESTS
  ^            ^                ^
  |            |                |
  W0 passes    T1-T7 trees     0.0.15 plan completeness checklist
  + notes      + subtrees      (plan/plan-0.0.15-uia-probe-and-routing.md)
```

No W1/W2 code, no test, no version bump and no Release chain is started for
0.0.15. The last shipped version stays `v0.0.14`; all four version sources
read `0.0.14`. "Three思后行" (think three times before acting) and "分而治之"
(divide and conquer) are the governing discipline of this phase: a large
branch gets its own subtree document and an explicit unlock condition before
any implementation is considered.

## Plan-completeness checklist for 0.0.15 (all must be true before W1)

1. The probe's exact COM surface is written down: CLSID/IID values, the
   vtable slots to be called, the `RECT` layout, and the HRESULT pass/fail
   rule.
2. The downgrade branch is written down: what happens on a hung provider, a
   zero rectangle with a successful HRESULT, or a missing pattern.
3. The reader contract is written down: which new evidence rows are additive,
   which earlier assertions stay, and what rejects a UIA scan.
4. The three-OS behaviour is written down: Windows does the probe; macOS /
   Linux / fallback keep their named reasons and empty trees.
5. The non-goals and the G boundary are restated; no `[no]` is lifted by the
   plan.
6. Only then does W1 (a Rust diagnostic example) become eligible to start.

## Learning notes (dated, process not just conclusions)

| Date | Pass | What was queried | Source class | Conflict / caution recorded | Went to |
|---|---|---|---|---|---|
| 2026-11-06 | 1 | L0-L3 framing and self-hosted orchestration | X (this repo) + S | L2 needs A5/D2 + B7b/D4 + A4 + F3, not one feature | `PRD_01_12`, T5 |
| 2026-11-06 | 2 | UIA gate downgrade to "probe required" | O / S | a published vtable order is not a verified slot on this machine | `PRD_01_17`, C1 |
| 2026-11-06 | 3 | candidate ordering by unlock condition | X + S | ordering is a reason to consider first, never a version commitment | T7 |
| 2026-11-06 | 3 | owner instruction to slow down before code | owner direction | earlier "implement next" delegation is superseded for this phase | this file, current phase |

## Non-goals

- `[no]` declaring any new `[v]`, capability row, ABI bump or matrix change
  from a study pass.
- `[no]` starting 0.0.15 W1/W2 code or tests before the checklist above is
  complete.
- `[no]` widening `screen` into host resources; C8 remains `[decision]`.
