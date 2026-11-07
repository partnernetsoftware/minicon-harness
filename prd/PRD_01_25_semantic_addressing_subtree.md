# PRD 01.25 — semantic addressing subtree (T9)

Status: **shipped in 0.0.15 with three-OS CI + owner-view acceptance**. Owner of
**how a computer-use target is addressed by identity instead of by a guessed
coordinate**. This is a new dimension of the PRD tree, split out because the
branch is large (perception, admission, actuation, evidence and the L3 end
state all cross it). Index: `PRD.md`. Learning ledger: `PRD_01_17` (W0 pass 7).
Contract: `plan/plan-0.0.15-a11y-addressed-actuation.md`.

T9 does not widen the domain. `PRD_01_08` still owns K/V/M/W/S and G; a
semantic node is a **window/control target**, not a host resource. No new
tool, verb, grant, crate, ABI or matrix cell exists here.

## Why this branch exists (X-grade, this machine, 2026-11-07)

`examples/cu_orchestrate.rs` drove the repo's own `CuHost::invoke_json` wire on
a real Windows desktop — no Python, no ctypes, no screenshot, no UIA:

| Request | Verdict | What came back |
|---|---|---|
| `{"op":"observe"}` | `performed` | 16 windows, `0,0 1680x1050`, focused `WorkBuddy` hwnd `33689578` |
| `{"op":"observe","a11y":33689578}` | `performed` | 43 MSAA nodes, 8 `button`, 11 named, 42 non-zero rects |
| `{"op":"observe","controls":33689578}` | `refused` | no Win32 child window on the self-drawn Electron client |

Named, bounded, operable buttons on the L3 target: `最小化`, `最大化`, `关闭`,
`对话内搜索`, `分享`, `打开右侧面板`, `新建任务`, `窗口置顶`. The shipped
0.0.11/0.0.14 MSAA tree **already saw** them; `act:control` could only resolve
`controls[]`, which is empty on that window. So the desktop was observable but
not addressable — the exact gap between L1 ("evidenced interaction") and L3
("self-hosted orchestration of this client"). T9 closes the addressing half.

This also corrects the 0.0.15 W1 UIA verdict's scope: "zero addressable
controls" held for the **UIA** provider on a lazily-activated Chromium tree,
not for the MSAA provider. `plan-0.0.15-uia-probe-and-routing.md` stays the
verbatim diagnostic record; the route switch is dated in `PRD_01_24`.

## T9 — semantic addressing tree

```text
T9 Semantic addressing (identity -> bounded effect -> readback)
├── S1 identity source
│   ├── [v] Win32 controls[] by label/class-role        0.0.5 / 0.0.6
│   ├── [v] MSAA a11y_controls[] normalised node        0.0.11 / 0.0.14
│   ├── [v] hwnd as a live window identity               0.0.12
│   └── [no] pixel coordinate as an identity             (a point is not a name)
├── S2 resolver                                          0.0.15
│   ├── [v] exact name wins, then unique name/value substring
│   ├── [v] optional `role` filter over a fixed vocabulary
│   ├── [v] ambiguity refuses with candidates named
│   ├── [v] invisible / offscreen / unavailable vetoed by state bits
│   ├── [v] unknown role refused, not ignored
│   └── [v] `role` with the Win32 arm refused, not silently dropped
├── S3 admission
│   ├── [v] `source` enum ("controls" default / "a11y") in CAPABILITIES + wire
│   ├── [v] frame + generation check before any pointer exists
│   ├── [v] empty a11y tree = named `no_a11y_tree` refusal, never a fallback
│   └── [v] press=false default: resolving a target cannot click a button
├── S4 effect
│   ├── [v] hover / click through the same 0.0.6 backend, one per call
│   ├── [v] centre must lie inside the virtual desktop
│   ├── [v] pointer readback must land inside the resolved node rect
│   └── [no] native default-action dispatch (study, S7)
├── S5 evidence
│   ├── [v] `source`, `a11y_node{name,value,role,role_name,state,state_names,
│   │        rect[4],depth,child_id}`, `a11y_node_schema`
│   ├── [v] `state_names` recomputed by the independent reader from `state`
│   ├── [v] reader recomputes centre and containment from published values
│   └── [v] every 0.0.6-0.0.14 assertion retained unchanged
├── S6 three-OS behaviour
│   ├── [v] Windows: MSAA tree real, semantic hover performed on real button
│   ├── [v] macOS / Linux / fallback: named tree/capability refusal
│   ├── [v] 12/10/8/0 matrix unchanged; four-cfg type gate passes
│   └── [v] independent journey reader grades the producer
├── S7 native action dispatch                            [study]
│   ├── MSAA `accDoDefaultAction` (Windows)
│   ├── `AXUIElementPerformAction` / `kAXPressAction` (macOS)
│   ├── `org.a11y.atspi.Action.DoAction` (Linux, D-Bus)
│   ├── unlock: three-OS probes, grant tiers, an effect-verification rule
│   └── caution: press-by-identity has no pointer readback; this repo's
│       governing invariant requires a readback, so a new evidence kind is
│       needed before any of it can become a capability
└── S8 relation to the maturity ladder
    ├── [v] L1 addressable-target interaction              0.0.15
    ├── [_] L2 self-sustained loop (gates A5/D2, F3 still open)
    └── [_] L3 self-hosted orchestration of this client
         └── S1-S6 remove the "cannot address the target" gate; the remaining
             gates are session management, effect evidence and hand-off
```

## Dimension cross-links

| Dimension | Tree | What 0.0.15 changes |
|---|---|---|
| Perception | T1 (`PRD_01_19`) | P2 gains an **addressable** a11y node, not only a readable one |
| Actuation | T2 (`PRD_01_20`) | B4 named semantic control now has two identity sources; A grammar gains `source`/`role` admission |
| Resource | T3 (`PRD_01_21`) | none; G stays closed, no process/CPU/GPU verb |
| Platform | T4 (`PRD_01_22`) | PL1 gains a real MSAA-target journey; 12/10/8/0 unchanged |
| Maturity | T5 (`PRD_01_23`) | L1 is now addressable; L2/L3 gates unchanged |
| Evidence | T6 (`PRD_01_22`) | E2 reader recomputes the node schema on act evidence |
| Version | T7 (`PRD_01_23`) | V0.0.15 becomes a shipped Release |
| Authority | T8 (`PRD_01_24`) | dated controller decision for the route switch |

## W0 pass 7 learning notes (grade O / P / S, dated 2026-11-07)

These are **study notes**, not capabilities. They explain why S2-S5 have the
shape above and which later gates remain open.

1. **The 2026 consensus is "addressable target, then verify".** Surveys and
   benchmark analyses (`OSWorld` action/observation spaces, grounding
   playbooks) report that semantic grounding is comparatively easy while
   *spatial* grounding — producing the pixel of a button on this rendering,
   at this DPI — dominates failures. At 97% per-step reliability a 40-step
   task succeeds 29.6% of the time; addressable handles, keyboard actions and
   an explicit check after every action are the levers, not a better model.
   **Caveat (recorded):** those percentages are model/stack-specific, and a
   PyAutoGUI "one step" can contain a whole control structure, so cross-paper
   success rates are not a controlled A/B. This repo therefore claims no
   success-rate improvement; it claims one more addressable target type.
2. **Accessibility trees are not complete semantic descriptions.** OSWorld's
   own observation notes say trees may be extremely long, incomplete,
   redundant or inaccurate; screenshots have the opposite limitation. The
   strong pattern is hybrid: prefer the a11y handle where it exists and
   cross-check so a stale tree does not point at a moved element. This repo
   has no image arm (A5 `[no]`), so it implements the **cross-check** half via
   generation pinning and a pointer readback inside the node rect, and states
   plainly that it does not do visual grounding.
3. **Native action dispatch exists on all three OSes** (`accDoDefaultAction`,
   `AXUIElementPerformAction(kAXPressAction)`, `atspi.Action.DoAction`) and can
   press without a pointer, focus or foreground activation. But an invoke has
   **no pointer readback**, which is this repo's entire notion of evidence.
   S7 therefore stays `[study]`: shipping it would require a new evidence
   kind, not just a new FFI call.
4. **The Windows MSAA/UIA split is not "old vs new".** Flutter and Chromium
   documents show both `IAccessible` (MSAA, plus `IAccessibleEx`) and
   `IRawElementProviderFragment` (UIA) implemented by the same node, with UIA
   patterns optional; which tree is populated is a **provider and launch-mode
   question**, not an API-version question. That is why a successful
   `CoCreateInstance` could coexist with an unusable UIA tree while MSAA on the
   same hwnd produced real buttons.
5. **Linux and macOS gates remain real.** AT-SPI2 needs a session bus and
   `toolkit-accessibility`/`QT_ACCESSIBILITY`; Wayland is semantic-only with
   portal-mediated input. macOS binds two separate TCC grants to the
   responsible process and its signature. None of these are `screen` verbs.
6. **Resource-domain cross-link.** `at-spi2-core` fd exhaustion can wedge a
   D-Bus portal into an `accept`-EMFILE spin and make every tree walk hit the
   full 15 s timeout: a stale a11y tree can be a **host resource symptom**.
   This is a reason G stays closed, not a grant of process/fd authority.

## Non-goals

- `[no]` an image arm, Set-of-Marks, OmniParser-style visual grounding or any
  screenshot-derived coordinate in this version (A5).
- `[no]` UIA write/invoke patterns, event subscription or provider changes.
- `[no]` batched `actions[]`, app-level effect evidence, a model loop or a
  second tool.
- `[no]` widening `screen` into host resources; G direction (a) is decided.
- `[no]` treating a study note as a decision, or a decision as evidence.
