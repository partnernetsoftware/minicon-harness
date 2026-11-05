# PRD 01.09 — richer actuation: path drag and two-axis scroll

Status: `[v]` shipped as the public company-signed `v0.0.7` (2026-11-05),
qualified on three OSes and accepted from an anonymous download. Owner of the
**contract** for 0.0.7;
`plan/plan-0.0.7-richer-actuation.md` owns the work breakdown, gates and
release chain. Nothing here claims a capability that a CI run has not
produced. `PRD_01_03` owns the `screen` tool's bounds,
`PRD_01_04` owns the host contract, `PRD_01_07` owns the actuation receipt,
`PRD_01_08` owns the domain model. This PRD only widens two declared verbs.

Legend: `[v]` delivered with evidence, `[-]` partial/open, `[_]` planned,
`[no]` explicit non-goal.

## Domain fit

Both changes are pure **M (mouse)** effects inside the KVM domain of
`PRD_01_08`. They add no resource-management authority, no process, memory,
device or host-control surface, and no harness-side capability. They do not
touch the loop, the wire codec's message shapes or the model's turn budget.

## Problem

0.0.6 proved that a button event was really dispatched. It did not change
*where* a drag can go or *which way* a wheel can turn: `act:drag` still
interpolates one straight line from `(x,y)` to `(to_x,to_y)` in 12 steps, and
`act:scroll` still has one vertical `delta`. OpenAI's `computer` tool has
`drag.path[]` and `scroll_x`/`scroll_y` (`prd/prd-cu-openai.md`, external
reference). Those two are the parts of B6/B7 that a three-OS CI journey can
honestly qualify today; the other halves are deliberately deferred (plan
0.0.7 decision table).

## Contract

### Additive wire parameters

```text
act:drag    { action:"drag", x, y, [to_x, to_y], [path: [[x,y], ...]], [button] }
act:scroll  { action:"scroll", x, y, [delta], [delta_x, delta_y] }
```

- `path` is optional. Omitting it keeps 0.0.6's `to_x/to_y` behaviour exactly,
  including the 12 stepped moves and the `pointer at X,Y` readback.
- `delta` remains accepted as a vertical alias for `delta_y` so existing
  callers, the 0.0.6 journeys and the `scrolled ... at X,Y` reader assertion
  keep working. Supplying both `delta` and `delta_y` is a named refusal.
- No new capability name, no new `CuVerdict`, no ABI major bump. `cu-abi 1.0`
  is unchanged; `CAPABILITIES` gains two typed parameter rows and two tests
  that the wire schema agrees with the manifest.

### Bounds, each a named constant beside the code that enforces it

| Constant | Value | What it bounds |
|---|---|---|
| `SCREEN_TOOL_MAX_DRAG_PATH_POINTS` | 16 | points in one `path[]`; a longer path is refused, never truncated |
| `SCREEN_TOOL_MAX_DRAG_STEPS` | 12 | stepped moves **per segment**, so a path stays bounded |
| `SCREEN_TOOL_MAX_SCROLL_DELTA` | 1440 | absolute value of each axis (`120 * 12`), matching the existing wheel clamp |

### Admission (before any native effect)

1. `path`, when present, is a non-empty array of at most 16 `[x,y]` integer
   pairs. One point, a non-array, a non-integer or an oversized array is a
   typed refusal naming the bound.
2. Every point lies inside the virtual desktop reported by the current
   observation's geometry. A point outside is refused with **both** rectangles
   named, never clipped by the OS to a different place.
3. `delta_x` / `delta_y` (or the `delta` alias) are integers within
   `±SCREEN_TOOL_MAX_SCROLL_DELTA`, and at least one axis is non-zero.
   `(0,0)` is a named refusal, never a silent no-op.
4. The existing admission order is unchanged: ABI major -> advertised
   capability -> grant -> platform capability -> `as_of` -> typed bounds ->
   native effect. A newer capability parameter cannot widen authority.

### Native surface (four configurations, identical signatures)

```text
native::drag(x, y, points: &[(i32,i32)], button) -> (px, py, ButtonDispatch)
native::scroll(x, y, delta_x, delta_y)          -> (px, py)
```

- Windows: `mouse_event` with `MOUSEEVENTF_HWHEEL` for the horizontal axis;
  the signed delta convention is normalised in evidence.
- macOS: `CGEventCreateScrollWheelEvent2` (non-variadic) through the existing
  `dlopen`/`dlsym` helper; no fixed Rust fn pointer calls a C variadic.
- Linux: XTest buttons 4/5 (vertical) and 6/7 (horizontal); button numbers
  never leak into evidence.
- fallback: identical signatures, named `unsupported` refusals.
- `scripts/cu-cfg-check.py` type-checks all four configurations in the same
  commit as the signature change.

### Evidence

- `act:drag` with a path keeps `evidence["actuation"]` with
  `button/downs/ups/double/target` exactly as 0.0.6 requires, and adds one
  additive integer `path_points` equal to the number of points actually
  traversed. The pointer readback must land at the **last** point within
  `SCREEN_TOOL_POINTER_TOLERANCE_PX`; otherwise `not_performed` / `unknown`.
- `act:scroll` posts no button event and therefore carries **no** actuation
  receipt. It gains `evidence["scroll"] = { delta_x, delta_y }`, normalised
  cross-platform. Its `readback` keeps the `scrolled ... at X,Y` shape, so
  the independent reader's coordinate regex still matches the pointer, not
  a delta.
- A receipt is still a claim about dispatch, not about an application's
  business action (`PRD_01_07`). A horizontal wheel in an Xvfb `xmessage`
  window has no observable application effect; the tool does not claim one.

## Verdict rules

| Situation | Verdict |
|---|---|
| path traversed, readback at last point, receipt matches | `performed` |
| receipt missing on a declared drag path | `unknown` |
| receipt `button`/`downs`/`ups` disagrees with the request | `not_performed` |
| path point outside the virtual desktop | `refused`, both rectangles named |
| both scroll axes zero, or `delta` and `delta_y` both supplied | `refused` |
| missing library / `$DISPLAY` / OS grant | `unsupported`, named |
| `as_of` pinned to an older generation | `stale` |

## Verification

- `src/cu.rs`: unit tests for path bounds, outside-desktop refusal, zero-axis
  refusal, alias conflict, receipt completeness and the additive
  `path_points`; the 0.0.6 mismatch/missing-receipt tests stay.
- `scripts/cu-cfg-check.py`: four configurations type-check with the new
  signatures.
- `.github/workflows/cu-platforms.yml`: three OSes, `cargo test --locked`,
  matrix still 12/10/8/0.
- `.github/workflows/cu-journeys.yml`: required `act_drag_path`,
  `act_scroll_x`, `act_scroll_y` on all three OSes. The independent reader
  recomputes the last path point, `path_points`, both scroll axes, the
  existing receipts and the strictly newer live final observation.
- No reader assertion is weakened to make a journey green.

## Non-goals

- `[no]` held-key modifiers, explicit bounded `wait`, batched `actions[]`.
  Each has an unlock condition in `plan-0.0.7-richer-actuation.md`.
- `[no]` a11y trees, image arm, Wayland-native protocol, runtime backend
  selection, app-level state-change evidence.
- `[no]` new crate, link-time platform library, engine, plugin host,
  bun/WASM/ujs embedding. PRD 31 readiness stays BLOCKED.
- `[no]` an ABI bump, a 13th capability row, or a change to the six false
  matrix cells; their unlock conditions live in `PRD_01_06`.

## Where this goes next

Only after W1-W4 of plan 0.0.7 are green (including the owner-view Release
acceptance) does the next version get planned.
The deferred halves of B6/B7 and B8 are candidates, not debts: each needs its
own PRD, its own gate and an owner decision before it enters a version.
