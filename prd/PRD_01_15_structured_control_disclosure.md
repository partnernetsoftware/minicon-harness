# PRD 01.15 — structured control disclosure (0.0.11)

Status: `[v]` implemented in `f0433d6`, awaiting the three-OS CI and the
public signed Release. Contract owner for 0.0.11;
`plan/plan-0.0.11-structured-control-disclosure.md` owns the work breakdown,
gates and release chain.

Legend: `[v]` delivered with evidence, `[_]` planned, `[no]` explicit
non-goal.

## Why this version exists

The L1 self-hosted orchestration trial drove the WorkBuddy client through CU.
`observe.controls[]` is `EnumChildWindows` + `WM_GETTEXT`: against a
Chromium/Electron window it reports exactly one
`Chrome_RenderWidgetHostHWND` with `role=unknown`, no input box and no send
button. The controller therefore fell back to screenshots plus a vision read
of pixel coordinates. That is slow, burns tokens, and is not evidence produced
by the CU contract. On 2026-11-06 the owner explicitly required that the
missing structured control read be built into 0.0.11 rather than guessed
around.

This is an **owner decision that lifts one `[no]` rule**, recorded here rather
than silently changed: earlier PRDs listed "a11y trees" as a non-goal. It is
lifted only to the bounded scope below. Every other `[no]` stays.

## Domain fit

KVM + W + S (`PRD_01_08`): this is **V (video/observation) and W (window
target binding)**, not host control. It reads a window's accessibility tree;
it does not manage processes, memory, devices or the network, does not post
input, does not request or change an OS grant, and never elevates. `PRD_01_12`
already records that an incomplete a11y arm is one of the hard blockers for
L2; this closes the Windows half of it.

## Wire contract (additive)

```text
observe { op:"observe", a11y: true }            -> foreground window
observe { op:"observe", a11y: 59771304 }        -> that live hwnd only
```

- `a11y` is a typed optional parameter of the existing `observe` capability,
  tag `a11y-scan`. `true` follows the foreground; an integer is a `hwnd` from
  a prior `observe.windows[]` and is the only unambiguous identity when two
  windows share a title. Anything else is a named `refused` before any effect.
- The handle is re-validated against a fresh live window list on every call.
  No live match publishes `a11y_reason`, never a guessed row.
- New evidence fields: `a11y_controls[]`, `a11y_source`, `a11y_reason`,
  `a11y_truncated`, `a11y_role_basis`. The existing `controls[]`,
  `controls_source` and `controls_role_basis` are unchanged.
- Each row: `name`, `role` (MSAA integer), `role_name` (fixed coarse table),
  `rect` in virtual-desktop coordinates (the same basis as `windows[]` and
  the PNG arm), `depth`, `child_id`.
- `cu-abi 1.0`, `CAPABILITIES`, the 12/10/8/0 matrix and the six false cells
  are unchanged. No new capability row, no new `CuVerdict`, no new crate.

## Bounds and honesty

- `SCREEN_TOOL_MAX_A11Y_CONTROLS = 400`, `SCREEN_TOOL_MAX_A11Y_DEPTH = 12`;
  hitting either sets `a11y_truncated`, never a silent truncation.
- Windows: hand-written MSAA/IAccessible COM bindings (`oleacc`/`ole32`),
  read-only, no new dependency. The Win64 `VARIANT` is 24 bytes; a 16-byte
  definition corrupts the stack and is covered by the type gate.
- Linux / macOS / fallback: no AT-SPI or AX tree is implemented. They publish
  an **empty** `a11y_controls[]` plus a named `a11y_reason`. They never
  present a Win32-style inventory as an accessibility tree.
- A refused scan with rows, or a successful scan with no rows and no reason,
  is a reader failure, not a `performed` success.

## Verification

- `src/cu.rs`: role-table, `a11y` parsing (foreground / hwnd / junk), and
  evidence-field tests; all 0.0.6-0.0.10 tests stay (109 passing locally).
- `scripts/cu-cfg-check.py` types all four backend configurations.
- Three-OS CI requires `observe_a11y` addressed by a live `hwnd`; the
  independent reader recomputes every row's role, depth, name and rectangle,
  the bound, and the empty-tree-plus-reason rule. No earlier reader assertion
  is weakened.
- Local evidence: `a11y:59771304` published 44 rows (15 named, e.g. `搜索`
  `button` `[34,3,24,24]`, `最小化` `[702,0,46,30]`) with no capture;
  `a11y:true` with no foreground WorkBuddy window published `unknown` with a
  reason instead of guessing.

## Non-goals

- `[no]` model-visible image arm, explicit `wait`, batched `actions[]`,
  app-level effect evidence, safety policy, three-level confirmation.
- `[no]` AT-SPI / AX implementations in this version: those need their own
  grants and three-OS fixtures. They are `unknown` with a reason here.
- `[no]` `act:focus` by `hwnd` and `target_hwnd` readback: that is the
  original handle-addressed contract, moved to 0.0.12
  (`PRD_01_14`). The `a11y` hwnd parameter is a scan target only, not an act
  target and not a focus effect.
- `[no]` window-manager product behaviour; `minicon-winmgr` owns it.
- `[no]` widening `screen` into host control; the G boundary in
  `PRD_01_08` is unchanged.
- `[no]` re-publishing or overwriting any shipped v0.0.4-v0.0.10 asset.
