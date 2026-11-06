# PRD 01.13 — display scale / coordinate-basis disclosure

Status: `[_]` specified after `v0.0.9` was published, **not implemented**.
Owner of the **contract** for 0.0.10; `plan/plan-0.0.10-display-scale-
disclosure.md` owns the work breakdown, gates and release chain. Nothing here
claims a capability that a CI run has not produced.

Legend: `[v]` delivered with evidence, `[_]` planned, `[no]` explicit
non-goal.

## Domain fit

Inside KVM + W + S (`PRD_01_08`) this is **V (pixel arm) precondition state**:
it describes the geometry a screenshot is expressed in. It grants no resource
authority, manages no display, posts no input and never borrows the
`PRD_01_07` actuation receipt. `PRD_01_08` already lists displays and input
devices as preconditions rather than managed objects; this PRD publishes one
more property of those preconditions.

`PRD_01_12` defines L2 as the lowest level at which a self-sustained
`observe -> decide -> act -> wait -> re-observe` loop can close. That loop
needs a coordinate basis before it needs an image arm: a model cannot convert
a point it sees into a point it clicks without the scale.

## Problem

`observe` publishes `displays[]` and `desktop` in physical pixels, and
`capture` writes a PNG of the same desktop. A caller reading that image
through a model-visible channel sees a resized copy; on a 125% Windows host
its coordinates are about 1.25x smaller than the physical ones. The 0.0.9
self-hosted orchestration trial measured exactly this: pure screenshot
coordinates missed the WorkBuddy input box, while a UIA rectangle hit it.
Nothing in the wire says what the ratio is. 0.0.10 publishes it as typed,
read-only evidence instead of leaving every caller to guess.

## Contract

### Additive evidence

```text
disclose { op:"disclose" }
  -> evidence["environment"]["display_scales"]: [
       { index, logical_rect, physical_rect, scale, reason? }, ...
     ]
observe  -> evidence["displays"][i].scale
```

- `logical_rect` / `physical_rect` are the `{x,y,width,height}` shape already
  used by `observe`; `scale` is `physical_width / logical_width` expressed in
  a bounded typed set (1, 1.25, 1.5, 2, 3 or `unknown`).
- A cell that cannot answer safely publishes `scale: "unknown"` with a
  non-empty `reason` naming the OS and the symbol that failed. It never
  publishes a plausible `1`.
- Rows are indexed by the existing `displays[]` order; no display id, device
  name, path or user name is published.
- No new capability name, no new `CuVerdict`, no ABI bump. `cu-abi 1.0`,
  `CAPABILITIES` and the 12/10/8/0 matrix are unchanged.

### Bounds

| Constant | Value | What it bounds |
|---|---|---|
| `SCREEN_TOOL_MAX_DISPLAY_SCALES` | 16 | rows one disclosure may publish |
| `SCREEN_TOOL_MAX_SCALE_REASON` | 160 | characters of one reason |

### Admission and honesty

1. `disclose` stays pure information; the `CuKind::Disclose` exemption stands.
2. No probe may change a display mode, DPI setting or process DPI awareness,
   and none may request a permission.
3. A probe that cannot be evaluated on the compiled cell is `unknown` with
   the cell named, never omitted.
4. `physical_rect` must equal `logical_rect * scale`; the independent reader
   recomputes it. A contradiction is `not_performed`.
5. `CuHost::drop` and every effect are untouched. Nothing here holds state
   across calls.

### Native surface (four configurations, identical signatures)

```text
native::display_scales() -> Vec<DisplayScaleProbe>
```

- Windows: `GetDpiForSystem` / `GetDpiForWindow` with `GetSystemMetricsForDpi`,
  read-only; no `SetProcessDpiAwareness`.
- macOS: `backingScaleFactor` resolved through the existing dlopen/dlsym
  helper; a missing symbol is `unknown`.
- Linux: read-only `Xft.dpi` / `XRRScreenResources` query; pure Wayland is
  named as `unknown`.
- fallback: one `unknown` row with the cell named.
- `scripts/cu-cfg-check.py` types all four configurations in the same commit.

## Verdict rules

| Situation | Verdict |
|---|---|
| disclosure published with well-formed scale rows | `performed` |
| a cell could not probe safely | `performed` with that row `unknown` + reason |
| a scale is outside the typed set or a reason is missing | `not_performed` |
| `physical_rect != logical_rect * scale` | `not_performed` (reader) |
| `--allow-screen` was never granted | `performed`; information, not an effect |

## Verification

- `src/cu.rs`: unit tests for the typed scale set, the reason rule, the row
  bound, index ordering, the ungranted disclosure path and the absence of a
  new capability row. All 0.0.6-0.0.9 tests stay.
- Three-OS CI journeys add a required `disclose_scales` step; the independent
  reader recomputes every row plus all earlier assertions.
- No reader assertion is weakened to make a journey green.

## Non-goals

- `[no]` model-visible image arm, `wait`, batched actions, a11y trees,
  app-level effect evidence, safety policy, Wayland-native protocol.
- `[no]` requesting or changing an OS grant, display mode or DPI setting.
- `[no]` new crate, runtime, engine, plugin host, bun/WASM/ujs embedding.
- `[no]` ABI bump, 13th capability row or any change to the six false cells.
