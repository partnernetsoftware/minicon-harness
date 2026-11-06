# PRD 01.14 — handle-addressed window targeting

Status: `[_]` specified after `v0.0.10` was published as a public signed
Release, **not implemented**. Owner of the **contract** for 0.0.11;
`plan/plan-0.0.11-handle-addressed-targeting.md` owns the work breakdown,
gates and release chain. Nothing here claims a capability that a CI run has
not produced.

Legend: `[v]` delivered with evidence, `[_]` planned, `[no]` explicit
non-goal.

## Domain fit

Inside KVM + W + S (`PRD_01_08`) this is **W (window target binding)**: it
names which window an act is bound to. It is not a window manager, grants no
resource authority, does not enumerate processes for management, posts no new
kind of input and never borrows the `PRD_01_07` button actuation receipt.
`PRD_01_08` already keeps window management product behaviour out of
`screen`; this PRD only makes the existing `act:focus` target unambiguous.

`PRD_01_12` defines L2 as the lowest level at which a self-sustained
`observe -> decide -> act -> wait -> re-observe` loop can close. A controller
that manages several session windows needs a stable per-window identity
before it needs more pixels.

## Problem

`observe.windows[]` publishes an `hwnd` per window, but `act:focus` accepts
only a unique `window` title. In the L1 self-hosted orchestration trial two
WorkBuddy client windows were both titled `WorkBuddy`; the controller could
not say which session window to bring forward and resorted to clicking pixel
coordinates. Rect geometry cannot disambiguate two same-titled windows, and a
coordinate basis (0.0.10) does not help: the failure is one of identity, not
of scale. Nothing in the wire lets a caller bind an act to a handle or read
back which handle was actually focused.

## Contract

### Additive evidence and parameters

```text
act { op:"act", action:"focus", why:"...", hwnd: 59771304 }
  -> evidence["actuation"] / focus readback gains "target_hwnd": 59771304
act { op:"act", action:"focus", why:"...", window:"WorkBuddy" }
  -> unchanged behaviour; readback still publishes target_hwnd
```

- `hwnd` is an integer taken from a prior `observe.windows[]` row.
- `window` (title) and `hwnd` are mutually exclusive. Giving both, or neither,
  is a named `refused` before any native effect.
- `target_hwnd` is the handle of the desktop's focused window at dispatch
  time. It is read from the live window list, never echoed from the request.
  A desktop that exposes no foreground window publishes `null` with a named
  reason, never a plausible handle.
- Rows are not republished: no new capability name, no new `CuVerdict`, no ABI
  bump. `cu-abi 1.0`, `CAPABILITIES` and the 12/10/8/0 matrix are unchanged.

### Bounds

No new numeric bound is introduced. The handle is validated by the existing
`SCREEN_TOOL_MAX_WINDOWS` window list; a handle outside that list is a named
refusal.

### Admission and honesty

1. `act:focus` stays an effect; the `--allow-screen` grant and the ordinary
   act admission rules are unchanged.
2. Every call re-validates the handle against the live window list *and* the
   window's current title. A stale, reused or mismatched handle is `stale` or
   `not_performed`; the host never falls back to another window silently.
3. A backend that cannot resolve handles publishes `unknown` with the cell
   named, never an omitted field.
4. `target_hwnd` must equal the focused window in the same observation; the
   independent reader recomputes it. A contradiction is `not_performed`.
5. `CuHost::drop`, held-key state and every other effect are untouched.
   Nothing here holds state across calls.

### Native surface (four configurations, identical signatures)

```text
native::window_by_handle(handle: u64) -> Option<WindowInfo>
```

- Windows: resolve through the existing `EnumWindows` list and `IsWindow`.
- macOS / Linux: resolve through the existing window enumeration; a backend
  without stable handles returns `None`, which the host reports as `unknown`
  with the cell named.
- fallback: `None` with the cell named.
- `scripts/cu-cfg-check.py` types all four configurations in the same commit.

## Verdict rules

| Situation | Verdict |
|---|---|
| handle resolved, focused, readback matches | `performed` |
| both `window` and `hwnd`, or neither | `refused` (named) |
| handle absent from the live list or title changed | `stale` / `not_performed` |
| backend cannot resolve handles | `performed` with named `unknown` reason |
| `target_hwnd` mismatched or missing without a reason | `not_performed` (reader) |
| `--allow-screen` was never granted | `refused`; focus is an effect |

## Verification

- `src/cu.rs`: unit tests for mutual exclusion, stale-handle refusal, the
  readback field, the reason rule, the ungranted path and the absence of a new
  capability row. All 0.0.6-0.0.10 tests stay.
- Three-OS CI journeys add a required `focus_by_handle` step plus a
  stale-handle refusal; the independent reader recomputes `target_hwnd` plus
  all earlier assertions.
- No reader assertion is weakened to make a journey green.

## Non-goals

- `[no]` model-visible image arm, `wait`, batched actions, a11y trees,
  app-level effect evidence, safety policy or Wayland-native protocol.
- `[no]` window-manager product behaviour; `minicon-winmgr` owns it.
- `[no]` requesting or changing an OS grant, display mode or DPI setting.
- `[no]` new crate, runtime, engine, plugin host, bun/WASM/ujs embedding.
- `[no]` ABI bump, 13th capability row or any change to the six false cells.
