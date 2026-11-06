# PRD 01.13 — display scale / coordinate-basis disclosure

Status: `[v]` **delivered in 0.0.10 with three-OS CI evidence**: the compile
gate `42177115600` and the journeys `42177115725` are green, and the
controller's independent reader recomputed the three `cu-journey.json`
artifacts as PASS (`scale` = `1` / `1` / `unknown` on macOS / Windows /
Linux, with the Linux `unknown` carrying a reason). Owner of the **contract**
for 0.0.10; `plan/plan-0.0.10-display-scale-disclosure.md` owns the work
breakdown, gates and release chain. `v0.0.10` is a public signed Release
(`e0e2671`, chain ending in publish `42184961997`); the owner-view acceptance
recomputed the 6 sidecars and 3 signed cells and ran the downloaded APE,
which prints `minicon-harness 0.0.10`. Nothing here claims a capability that
a CI run has not produced.

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

`observe` publishes `displays[]` and `desktop`, and `capture` writes a PNG
of the same desktop; the wire did not previously state the coordinate basis
that relates a point a model reads in an image to the point a pointer event
needs. 0.0.10 publishes that basis as typed, read-only evidence instead of
leaving every caller to guess. This is a **precondition** for A5 (the
model-visible image arm) and for cross-DPI / multi-monitor / scaled sessions,
not the fix for a measured DPI fault. An earlier L1 trial note claiming a
~1.25x screenshot/desktop mismatch was a controller misreading: CU
self-reports `virtual_desktop`, `desktop` and PNG geometry consistently at
1680x1050 with a self-consistent coordinate system, a click taken from CU's
own image hit the target on the first try, and three-OS CI measures `scale`
as `1` / `1` / `unknown` (macOS / Windows / Linux). The earlier missed clicks
came from mixing UIA, PIL and CU coordinate systems with focus never reaching
the input box, not from a display scale.

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
  reader recomputes every row (six-value whitelist, `physical_rect ==
  logical_rect * scale` in exact integer arithmetic, `unknown` with a
  non-empty reason, 0-based continuous indices, 16-row and 160-char bounds)
  plus all earlier 0.0.6-0.0.9 assertions. Runs `42177115600` / `42177115725`.
- No reader assertion is weakened to make a journey green.

## Non-goals

- `[no]` model-visible image arm, `wait`, batched actions, a11y trees,
  app-level effect evidence, safety policy, Wayland-native protocol.
- `[no]` requesting or changing an OS grant, display mode or DPI setting.
- `[no]` new crate, runtime, engine, plugin host, bun/WASM/ujs embedding.
- `[no]` ABI bump, 13th capability row or any change to the six false cells.
