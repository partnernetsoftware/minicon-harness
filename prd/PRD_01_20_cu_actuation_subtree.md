# PRD 01.20 — CU actuation subtree (T2)

Status: **study + structure, plus the 0.0.15 shipped branch** (owner direction
2026-11-06 evening: keep learning and structuring the PRD; **do not start
code or tests until the plan is complete**). M4a / A4a started only after
`plan/plan-0.0.15-a11y-addressed-actuation.md` passed 6/6; see `PRD_01_25`.
No new capability row, verb, grant, crate, `cu-abi` bump, `[no]` lift or G
change. Sibling subtrees: `PRD_01_19` (T1 perception),
`PRD_01_21` (T3 resource domain), `PRD_01_22` (T4/T6),
`PRD_01_23` (T5/T7). Learning ledger: `PRD_01_17_frontier_radar.md`.

Legend: `[v]`, `[_]`, `[no]`, `[study]` as in `PRD_01_19`. Evidence grades:
**O** official, **B** benchmark, **P** paper, **V** vendor, **S** survey,
**X** local measurement.

## T2 — actuation and input tree

```text
T2 Actuation / input
├── K keyboard                                                   [v] 0.0.6-0.0.8
│   ├── K1 `act:type` bounded 4096 chars                             [v]
│   ├── K2 `act:key` chords (ctrl+s, win+left)                      [v]
│   ├── K3 held modifiers: 4-whitelist, host-owned set, teardown    [v] 0.0.8
│   ├── K4 `disclose.held_keys` + `environment` / `grants[]`        [v] 0.0.9
│   └── K5 keyboard-layout / IME / dead-key guarantee                [no] not claimed
├── M mouse                                                      [v] 0.0.6-0.0.7
│   ├── M1 click / double / middle / right + button receipt         [v] 0.0.6
│   ├── M2 drag path 2..16 points, 12 steps/segment, path_points    [v] 0.0.7
│   ├── M3 two-axis scroll delta_x/delta_y, ±1440, evidence         [v] 0.0.7
│   ├── M4 hover / move / named control with press                   [v]
│   ├── M4a named a11y node by identity (`source`/`role`)             [v] 0.0.15
│   │        exact name -> unique substring -> role filter;
│   │        invisible/offscreen/unavailable vetoed; readback inside rect
│   └── M5 synthetic cursor per agent / background input             [no] here
├── W window target binding                                       [v] 0.0.12
│   ├── W1 `act:focus` by title (exact, then unique substring)        [v]
│   ├── W2 `act:focus` by `hwnd`, mutually exclusive with title       [v]
│   ├── W3 live re-validation + stale-handle `stale` refusal          [v]
│   └── W4 `target_hwnd` desktop readback, `actuation` stays null     [v]
├── A action grammar and receipts                                 [study]
│   ├── A1 one effect per call, required `why`                        [v]
│   ├── A2 six typed verdicts + generation / as_of / stale refusal    [v]
│   ├── A3 `evidence["actuation"]` button downs/ups/double/target     [v] 0.0.6
│   ├── A4 `verify_actuation`: missing receipt -> unknown             [v]
│   ├── A4a `source`/`role` admission refused before any pointer      [v] 0.0.15
│   │        `a11y_node` + schema published; reader recomputes state_names
│   ├── A5 batched `actions[]` with ordered receipts                   [_] B8
│   └── A6 post-action state verification / retry loop                [_] F3/L2
├── C code-first / hybrid execution (external)                     [study]
│   ├── C1 "emit a script, run it, return screenshot + output"        [no] here
│   └── C2 native app APIs preferred over simulated clicks            [_]
└── S safety of effects                                           [study]
    ├── S1 UIPI integrity levels / UAC secure desktop                  [v] named refusals
    ├── S2 macOS TCC (Accessibility / Input Monitoring)               [v] named boundaries
    ├── S3 screen-borne prompt injection -> untrusted on-screen text  [_] C6
    ├── S4 hard approval gate for destructive actions                 [_] C5
    ├── S5 held-input fail-closed release on teardown                 [v] 0.0.8
    └── S6 `pending_safety_checks` / human hand-off                   [_] C5/C6
```

## External findings, 2026-11-06 (third W0 pass)

- **The action grammar has converged** [S + O]: click, type, scroll, hotkeys,
  waits, with minor dialect differences across vendors. Two axes remain:
  perception channel (pixels / tree / hybrid) and action representation
  (coordinates / element references). This repo uses coordinates with a
  desktop-read receipt; ref-based actions (a11y element ids) are the
  alternative shape used by `xa11y` / `agent-desktop`.
- **Code-first is the emerging cost lever** [V + S]: a 2026 frontier offering
  put `computer_use` beside native code execution and recommends emitting a
  PyAutoGUI / Playwright-style script, executing it and returning the
  screenshot plus stdout — compressing multi-step actions, loops and
  conditionals into one call (CoAct-1 lineage). This is a **non-goal here**:
  `minicon-harness` is one bounded tool call per effect with no scripting
  surface over the desktop.
- **Input tiers are a platform design, not one API** [S + O]: a mature driver
  routes per capability — `ax` (UIA patterns / AT-SPI actions / AXPress)
  headless by default, `event` (SendInput / CGEventPostToPid / XTest or the
  Wayland RemoteDesktop portal) for coordinates, and `activate`
  (SetForegroundWindow + synthetic input) as an explicit fallback. Missing
  patterns must be reported, not silently converted into a different tier.
  Held-input commands should fail closed until a daemon can release them.
- **Screen content is untrusted input** [P + S]: CaMeLs / NOVA show that a
  single-shot trusted planner emitting a branching plan with control-flow
  integrity can retain up to 57% of frontier performance while blocking
  instruction injection, and even improve small open models by up to 19%;
  Branch Steering attacks still need extra defences. Production guidance is
  uniform: sandbox, least privilege, human approval for consequential
  actions, log every step, treat all on-screen text as untrusted.
- **Failure taxonomy belongs to the harness** [S]: mis-click on dense UI,
  silent-failure retry loops, hallucinated UI, state drift (cookies, popups,
  crashes), unapproved destructive actions, credential leakage via
  screenshots, prompt injection. Whether a failure is caught, contained and
  shown to a person is a harness property, not a model property.

## Node notes (repo mapping and unlock conditions)

| Node | What this repo already has | Unlock condition |
|---|---|---|
| A5 | strict one-effect-per-call admission | admission / stale-generation design for ordered receipts (B8) |
| A6 | `observe` readback + bounded wait (0.0.13) | side-effect-free three-OS app fixture (F3), then L2 |
| C2 | none; no connector or native app API layer | a routing PRD and a policy decision |
| S3 / S4 | `why`, grant gating, verdicts, receipts | C5/C6 policy PRD before any code |
| S6 | none | policy PRD + hand-off protocol |
| M5 | one physical pointer, foreground-based effects | a background-input PRD; explicitly out of the current plan |

## Learning notes (dated, process not just conclusions)

| Date | Pass | What was queried | Source class | Conflict / caution recorded | Went to |
|---|---|---|---|---|---|
| 2026-11-06 | 1 | Anthropic / OpenAI / Microsoft action surfaces | O / S / V | vendor "hold_key" and 12 ms inter-key pacing are product details, not a cross-vendor standard | `PRD_01_17` |
| 2026-11-06 | 3 | action grammar, input tiers, code-first | S / V / O | code-first claims come from vendor launches (V); no independent cross-vendor benchmark was found | this file, A/C/S |
| 2026-11-06 | 3 | prompt injection defences (CaMeLs / NOVA) | P | 57% retained performance is on OSWorld under a specific planner; not a general safety bound | `PRD_01_22`, this file |
| 2026-11-06 | 3 | `xa11y` / `agent-desktop` platform constraints | S / O | "no grant needed on Windows" is integrity-level conditional; headless ref actions still need a11y providers | `PRD_01_22` |

## Non-goals

- `[no]` batched actions, a scripting surface over the desktop, a second
  model loop, takeover / owner-lock, background synthetic cursors, or any
  application-level effect claim in the current plan.
- `[no]` widening `screen` into process / CPU / memory / GPU authority; G
  is owned by `PRD_01_08` and studied in `PRD_01_21`.
- `[no]` treating a vendor safety feature as a shipped capability. Every
  `[v]` above comes from this repo's three-OS CI and independent reader.
