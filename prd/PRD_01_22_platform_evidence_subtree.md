# PRD 01.22 — platform and evidence subtree (T4 + T6)

Status: **study + structure only** (owner direction 2026-11-06 evening:
learn and structure first; **do not start code or tests until the plan is
complete**). Changes no source, version, capability row, `cu-abi`, `[no]`
row or the G boundary. Sibling subtrees: `PRD_01_19` (T1), `PRD_01_20` (T2),
`PRD_01_21` (T3), `PRD_01_23` (T5/T7). Learning ledger: `PRD_01_17`.

Legend: `[v]`, `[_]`, `[no]`, `[study]` as in `PRD_01_19`. Evidence grades:
**O** official, **B** benchmark board, **P** paper, **V** vendor,
**S** survey / secondary, **X** local measurement.

## T4 — platform / OS grant tree

```text
T4 Platform and grants (12 / 10 / 8 / 0 matrix)
├── PL1 Windows                                              [v] 12/12
│   ├── PL1.1 EnumWindows + GetWindowText + pid + rect          [v]
│   ├── PL1.2 UIA control-view walker (external)                [no] pass 5 downgrade
│   ├── PL1.3 MSAA / IAccessible hand-written COM               [v] 0.0.11
│   ├── PL1.4 SendInput; UIPI equal-or-lower integrity rule    [v] named refusals
│   ├── PL1.5 UAC secure desktop is not clickable              [study]
│   ├── PL1.6 PrintWindow black frames -> WGC -> BitBlt        [study]
│   ├── PL1.7 Session 0 has no interactive desktop             [study]
│   └── PL1.8 DWM shadow rects / extended frame bounds         [study]
├── PL2 macOS                                                [v] 8/12
│   ├── PL2.1 AX tree / AXPress / AXUIElement                  [study] TCC gate
│   ├── PL2.2 TCC: Screen Recording / Accessibility /          [v] named boundaries
│   │          Input Monitoring / Automation
│   ├── PL2.3 stable signed bundle id for TCC grants          [study]
│   ├── PL2.4 ScreenCaptureKit window capture                  [study]
│   ├── PL2.5 CGEvent / CGEventPostToPid input tiers          [v]
│   ├── PL2.6 focus unsupported -> capability boundary        [v]
│   └── PL2.7 display scale via backingScaleFactor            [v] 0.0.10
├── PL3 Linux                                                [v] 10/12
│   ├── PL3.1 X11 XTest / Xvfb journey                        [v]
│   ├── PL3.2 AT-SPI2 over D-Bus; one round trip per attr     [study]
│   ├── PL3.3 a11y bus at session start, DISPLAY, atspi group  [study]
│   ├── PL3.4 Wayland: ScreenCast / RemoteDesktop portals,     [study]
│   │          libei, wl-clipboard; no native window control
│   ├── PL3.5 xcap GetImage / D-Bus / RandR capture           [study]
│   ├── PL3.6 a11y empty tree + named reason                  [v] 0.0.11
│   └── PL3.7 display scale `unknown` with reason             [v] 0.0.10
├── PL4 fallback cell                                        [v] 0/12
│   └── PL4.1 every capability row false, named refusal         [v]
└── PL5 cross-platform contract                              [v]
    ├── PL5.1 four configurations, identical native signatures  [v]
    ├── PL5.2 `cu-cfg-check.py` type gate in the same commit    [v]
    └── PL5.3 12 / 10 / 8 / 0 matrix unchanged since 0.0.6    [v]
```

## T6 — evidence and verification tree

```text
T6 Evidence / verification
├── E1 CI gates                                               [v]
│   ├── E1.1 cu-platforms: build, test, release-link 3 OSes      [v]
│   ├── E1.2 cu-journeys: real desktop, Xvfb / GUI / Notepad     [v]
│   └── E1.3 four-backend type gate + examples build             [v]
├── E2 independent recomputation                              [v]
│   ├── E2.1 `cu-journey-verify.py` grades, producer never does  [v]
│   ├── E2.2 no earlier assertion weakened to make green         [v]
│   └── E2.3 BOUNDARY note excuses only the named boundary       [v]
├── E3 typed evidence rows                                    [v]
│   ├── E3.1 button / scroll / held-key / target_hwnd receipts   [v]
│   ├── E3.2 environment, grants[], display_scales[]            [v]
│   ├── E3.3 a11y tree + normalised node schema                 [v]
│   ├── E3.4 wait: requested/elapsed/cumulative/calls/generation[v]
│   └── E3.5 generation / as_of / stale refusal / receipts       [v]
├── E4 release evidence                                       [v]
│   ├── E4.1 pack -> candidate -> signing -> dry-run -> publish  [v]
│   ├── E4.2 build-receipt + signing-receipt + 6 .sha256         [v]
│   ├── E4.3 three signed assets, Authenticode + RFC 3161        [v]
│   └── E4.4 owner-view: anonymous page/asset 200, run the APE   [v]
├── E5 external verification methods (learned, not adopted)   [study]
│   ├── E5.1 OSWorld 2.1 coordinated release contract           [study]
│   ├── E5.2 acceptance + rejection tests for a verifier        [study]
│   ├── E5.3 side-effect / forbidden-shortcut / credential audit [study]
│   ├── E5.4 explicit tool-permission record + run provenance   [study]
│   └── E5.5 OSWorld-Human efficiency reference trajectories    [study]
└── E6 evidence grades                                        [study]
    └── O / B / P / V / S / X; never promote V to [v]          [study]
```

## Findings, 2026-11-06 (third W0 pass)

- **Grants are granular and conditional** [O + S]: macOS needs four separate
  TCC permissions (Screen Recording for capture, Accessibility for the AX
  tree and Apple events, Input Monitoring for a kill-switch tap, Automation
  for System Events); a TCC grant belongs to a **stable signed bundle id**, so
  a bare binary has no stable identity. Windows UIA needs no extra grant for
  applications at the terminal's **equal integrity level**, but an elevated
  target accepts input only from an elevated agent; Session 0 has no
  interactive desktop and UAC's secure desktop cannot be clicked. Linux needs
  AT-SPI2 running with the session bus and `DISPLAY`, plus `atspi` group
  membership in some setups; a TTY / SSH shell without the session bus cannot
  reach desktop apps.
- **Wayland is a different capability set** [O + S]: semantic a11y actions
  work on X11 and Wayland, but Wayland lacks app pointer input and native
  window control; clipboard needs `wl-clipboard`; capture goes through the
  ScreenCast portal and event-tier input through RemoteDesktop / `libei`.
  A driver must route per capability at runtime and name the missing one
  rather than pretending X11 semantics.
- **One Windows API is never enough** [S + O]: `PrintWindow` returns black
  frames for DirectComposition / UWP surfaces (WGC reads the DWM-composited
  frame even when covered); UIA cache requests avoid thousands of cross-process
  property calls on a Chromium tree; VCL / SAL providers can hang on subtree
  caches and need the MSAA route; DWM rectangles include invisible shadows.
  A mature driver reports `capture_source` when it falls back to a cropped
  desktop image, because covered pixels may then be wrong.
- **Input tiers and fail-closed held input** [S + O]: `ax` (UIA patterns /
  AT-SPI actions / AXPress) runs headless without moving the pointer; `event`
  (SendInput / CGEventPostToPid / XTest or the Wayland portal) handles
  coordinates; `activate` (SetForegroundWindow + synthetic input) is an
  explicit fallback. Password controls must refuse `set_value` / typed input.
  Held key / button commands should fail closed until a daemon can release
  them, so a crashed caller cannot leave input stuck down.
- **Traces are sensitive output** [O + V]: snapshots, window titles, clipboard
  reads, notifications, screenshots and session traces can contain private
  data; redaction does not make a screenshot safe to publish. This is why
  this repo bounds captures to the task root and never ships a session trace
  as a public asset.
- **Verification is a release contract** [P, OSWorld 2.1]: the 2026-09-16
  2.1 release pins a coordinated version contract — environment/runner code,
  task implementations, complete assets, website source and deployment, VM
  image and runtime, task hashes and run provenance. Its manifest explicitly
  does **not** claim a full 108-task agent evaluation; it records verified
  image bytes / runtime digests without a successful Docker VM boot and an
  observed setup timeout on one task. A tag or a green smoke test does not
  establish that every task works. Human reference trajectories add an
  efficiency axis (extra steps and latency); a batched action and an atomic
  mouse operation cannot be counted as interchangeable units.

## Learning notes (dated, process not just conclusions)

| Date | Pass | What was queried | Source class | Conflict / caution recorded | Went to |
|---|---|---|---|---|---|
| 2026-11-06 | 1 | three-OS a11y APIs and TCC / UIPI / AT-SPI | O / S | "no grant on Windows" is integrity-level conditional | `PRD_01_17`, T4 |
| 2026-11-06 | 3 | `computer-use` crate doctor / input tiers / WGC | O (crate docs) / S | crate-level design choices are one implementation, not a platform contract | T4 |
| 2026-11-06 | 3 | `xa11y` / `agent-desktop` grants and privacy notes | O / S / V | Linux "nothing else to grant" assumes GNOME with AT-SPI running | T4 |
| 2026-11-06 | 3 | OSWorld 2.1 release contract and verifier design | P / O | manifest pins components but disclaims full evaluation; do not quote it as a leaderboard | T6 E5 |
| 2026-11-06 | 1-3 | this repo's own CI / release chain | X (local + CI runs) | owner-view acceptance is the only claim allowed to become `[v]` | T6 E1-E4 |
| 2026-11-06 | 5 | UIA provider behaviour across seven real windows | X (local probe) | all slots S_OK + usable desktop root RECT, but the WorkBuddy provider gives 8 Pane nodes / 0 non-Pane controls; cold-start zero RECTs settled to `840,0,1680,1014` on re-run. One successful COM call is neither a usable tree nor a stability claim | `plan-0.0.15`, T4 PL1.2 |

## Non-goals

- `[no]` Wayland-native protocol, runtime backend selection, a second model
  loop or an OS-grant request in the current plan.
- `[no]` relaxing the matrix, the six false cells, `cu-abi 1.0` or any
  reader assertion to make a journey green.
- `[no]` adopting OSWorld / WAA scoring as a claim about this product. The
  external methods are study material for E5 only.
