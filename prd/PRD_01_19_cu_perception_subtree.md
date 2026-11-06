# PRD 01.19 — CU perception subtree (T1)

Status: **study + structure only**. Owner direction (2026-11-06, evening):
keep learning the newest computer-use and resource-management technology, keep
the learning notes in the PRD, expand the PRD into multi-dimensional trees,
split large branches into subtree documents, and **do not start code or tests
until the plan is complete**. This document changes no source, version,
capability row, `cu-abi`, `[no]` row or the G boundary.

Owner of the **observation / perception branch**. Sibling subtrees:
`PRD_01_20_cu_actuation_subtree.md` (T2),
`PRD_01_21_resource_domain_subtree.md` (T3),
`PRD_01_22_platform_evidence_subtree.md` (T4 / T6),
`PRD_01_23_capability_maturity_subtree.md` (T5 / T7).
Dated learning ledger: `PRD_01_17_frontier_radar.md`.

Legend: `[v]` shipped with three-OS evidence, `[_]` candidate / planned,
`[no]` explicit non-goal, `[study]` research note with no implementation
claim. Evidence grades (defined in `PRD_01_17`): **O** official doc,
**B** benchmark board, **P** paper / preprint, **V** vendor claim,
**S** survey / secondary, **X** local measurement.

## T1 — observation and perception tree

```text
T1 Observation / perception
├── P1 pixel arm (screenshot -> coordinates)                       [study]
│   ├── P1.1 bounded desktop PNG, 16 MiB, scale 1..8                [v] 0.0.6
│   ├── P1.2 display-scale / coordinate-basis disclosure            [v] 0.0.10
│   ├── P1.3 per-window capture router (PrintWindow/WGC/BitBlt)    [_] A5 gate
│   └── P1.4 model-visible base64 image arm                        [no] A5/D2
├── P2 structured control arm                                      [study]
│   ├── P2.1 Win32 EnumChildWindows + WM_GETTEXT `controls[]`        [v] 0.0.5
│   ├── P2.2 Windows MSAA / IAccessible read-only tree               [v] 0.0.11
│   ├── P2.3 normalised node schema (state/value/schema tag)         [v] 0.0.14
│   ├── P2.4 UIA control-view walker + MSAA fallback                [no] W2 stopped
│   │        (pass 4: vtable + GUIDs verified against installed SDK;
│   │         pass 5: probe verified every COM slot on 6 windows, but the
│   │         target WorkBuddy provider gives 8 Pane nodes / 0 non-Pane
│   │         controls plus a cold-start zero-rect window; W2 stopped)
│   ├── P2.5 AT-SPI2 (Linux) / AX (macOS) trees                     [_] OS grants
│   └── P2.6 incremental a11y change subscription                   [_] 3-OS fixture
├── P3 hybrid / interface-first routing                            [study]
│   ├── P3.1 connector / native API / MCP before GUI                 [_]
│   ├── P3.2 a11y tree first, pixels as visual fallback               [_]
│   └── P3.3 code-first execution (emit a script, then run it)        [no] here
├── P4 target identity and binding                                 [v] 0.0.12
│   ├── P4.1 window inventory: hwnd / pid / rect / focused             [v]
│   ├── P4.2 handle-addressed focus + `target_hwnd` readback           [v]
│   └── P4.3 stale / reused / mismatched handle refusal                [v]
├── P5 timing and freshness                                        [v] 0.0.13
│   ├── P5.1 one bounded sleep, then a fresh snapshot                  [v]
│   ├── P5.2 per-call / cumulative / call-count ceilings                [v]
│   └── P5.3 condition wait, polling loop, model turn                  [no]
└── P6 grounding (external knowledge, not this repo's contract)    [study]
    ├── P6.1 ScreenSpot -> ScreenSpot-Pro accuracy cliff
    ├── P6.2 resolution budget as a procurement dimension
    ├── P6.3 dedicated grounding models (UGround/ShowUI/OmniParser)
    └── P6.4 bidirectional downscale / upscale discipline
```

## External findings, 2026-11-06 (third W0 pass)

- **Three perception routes, one converged decision order** [S + O]: pure
  pixels cost ~2.2-5 s per action and pay a permanent visual-grounding tax;
  DOM / a11y-tree input is ~150 ms class and 5-10x cheaper and more reliable
  on the same SaaS scenario; hybrid (tree first, pixels for verification)
  consistently beats either alone on OSWorld. The production rule is
  *API -> browser/DOM -> desktop pixels only when nothing else reaches it*.
  Mariner was stopped on 2026-05-04 and folded into Gemini / Chrome
  auto-browse; that is a retreat from *vision-as-default for the web*, not
  from pixels for native desktop apps.
- **Grounding is a variable, not a constant** [S + P]: ScreenSpot ~90%+ drops
  to ~37% on ScreenSpot-Pro as interface density rises. Resolution became a
  procurement dimension: one 2026 frontier model raised the long edge to
  2576 px with 1:1 coordinate mapping, replacing the earlier downscale /
  re-screenshot loop. Coordinates must be scaled **both ways** — downscale
  the capture, upscale the returned click — or the agent keeps clicking just
  below the button. Dedicated grounding models (UGround, ShowUI, OmniParser,
  UI-TARS) form their own layer.
- **The normalised node schema is the adapter contract** [S + O]: four native
  accessibility vocabularies (UIA control types, AX attributes, AT-SPI roles,
  ARIA roles) need one internal node schema — role, name, state set, value,
  bounds, relations — with per-platform adapters. `v0.0.14` shipped the
  first half of that schema (`role/name/state/value/bounds/depth`); P2.6 and
  P2.5 are its consumers, not replacements.
- **Trees are event streams** [S]: providers push property-changed and
  structure-changed events; polling full trees is wasteful and stale.
  Auto-waiting locators (Playwright-style, poll until the element appears or
  a timeout expires) are the ergonomic answer to async re-render. On Linux
  each attribute is a separate D-Bus round trip, so selector filters should
  be evaluated *during* the tree walk rather than after collecting nodes.
- **Verification has two failure modes** [P, OSWorld 2.0/2.1 construction]:
  acceptance tests alone allow false accepts; a single reference solution
  allows false rejects of valid alternatives. A verifier needs both
  acceptance and rejection tests, plus side-effect / forbidden-shortcut
  audits, credential-exposure checks and an explicit tool-permission record.
  A temporary state satisfying a check is not the terminal deliverable.

## Node notes (repo mapping and unlock conditions)

| Node | What this repo already has | Unlock condition |
|---|---|---|
| P1.3 | desktop capture with a named scale; `display_scales[]` | A5/D2 image arm + PNG budget and retention decision |
| P2.4 | MSAA tree, `a11y_source`, `a11y_reason`; CLSID/IID + exact vtable slots verified from the installed Windows SDK (`PRD_01_17` pass 4) | **closed by pass 5**: the target WorkBuddy provider must expose named, non-Pane, stable bounded nodes; the downgrade rule fired and W2 stopped |
| P2.5 | empty trees + named reasons on Linux / macOS / fallback | OS grants (TCC / AT-SPI bus) and three-OS fixtures |
| P2.6 | static bounded snapshot (400 rows / depth 12) | event subscription API on three OSes and a staleness contract |
| P3.1 / P3.2 | no connector; `screen` is the only desktop entrance | a routing PRD and a policy decision; not a version commitment |
| P4.3 | live `window_by_handle` on four configs | none, already shipped |
| P5.3 | explicit refusal of polling and condition waits | a loop/budget PRD; B7b was lifted only for one bounded sleep |

## Learning notes (dated, process not just conclusions)

| Date | Pass | What was queried | Source class | Conflict / caution recorded | Went to |
|---|---|---|---|---|---|
| 2026-11-06 | 1 | CU technology route and industry shape | O / B / S / V | OSWorld Verified ~90% vs 2.0 binary ~20-44%; partial vs binary, 100 vs 500 steps are not comparable | `PRD_01_17` |
| 2026-11-06 | 1 | UIA vs MSAA, WAA, UFO2 | O / P / S | WAA 8.6%->13.1% is one open-source perception stack, not a general gain | `PRD_01_17`, P2.4 |
| 2026-11-06 | 2 | `IUIAutomation` COM vtable order | O / S (Go binding, MS Q&A) | `RECT` (4x int) != `UiaRect` (4x double); swallowed HRESULT hides a failed read | `PRD_01_17`, P2.4 |
| 2026-11-06 | 3 | perception routes, grounding, verification | S / P / O | per-step 150 ms vs 2.2-5 s are different setups, not a controlled A/B; vendor scores are V | this file, P1-P6 |
| 2026-11-06 | 3 | cross-platform a11y APIs and grants | O / S / V | "Windows needs no grant" holds only at equal integrity level; Wayland needs portals | `PRD_01_22` |
| 2026-11-06 | 4 | installed Windows SDK `UIAutomationClient.h`/`.idl`: CLSID/IID, exact vtable slots, `RECT`, `TreeScope` | O (installed SDK 10.0.22000.0) | `RECT` = left/top/right/bottom, not `[x,y,w,h]`; a header does not prove runtime provider behaviour; raw HRESULTs still required | `PRD_01_17` pass 4, P2.4, `plan-0.0.15` |
| 2026-11-06 | 4 | decision authority correction under `/goal` | owner direction | a reversible default must not be escalated; owner keeps a veto | `PRD_01_24`, T8 |
| 2026-11-06 | 5 | W1 UIA probe on the real desktop | X (this machine) | all slots S_OK + usable desktop root RECT, but the target provider gives 0 non-Pane controls and cold-start reads were all-zero RECTs; a callable interface is not a useful provider, and one read is not stability | `PRD_01_17` pass 5, P2.4 |
| 2026-11-06 | 6 | Electron/Chromium lazy `AXMode` activation | O (Electron docs) / S | the empty tree is a target-app launch precondition (`--force-renderer-accessibility` / AT present), not a reader bug and not something `screen` may enable | `PRD_01_17` pass 6, P2.4 reopen trigger |

## Non-goals

- `[no]` a model-visible image arm, a screenshot-vision coordinate read, a
  connector / MCP routing layer or code-first script execution in the current
  plan. The owner rejected screenshot-and-vision reads as slow and token-heavy.
- `[no]` any UIA pattern invocation, event subscription or write through the
  accessibility surface: the tree stays read-only.
- `[no]` claiming a perception capability from a benchmark score. Only
  three-OS CI plus the independent reader produces `[v]`.
