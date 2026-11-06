# 0.0.15 — UIA probe verdict, W2 stopped by the downgrade rule

Status: `[v]` **W1 delivered and run on a real Windows desktop**;
`[no]` **W2 stopped** by the mandatory downgrade rule below. Version sources
stay at `0.0.14`, no Release chain is started, and no shipped source path
changed. W0 pass 4 completed the plan (6/6) on 2026-11-06; W0 pass 5 recorded
the probe verdict. The controller decided W2's stop itself (`PRD_01_24`, T8)
and did not offer the choice back to the owner.

Baseline: `v0.0.14` published the normalised a11y node schema on top of the
0.0.11 Windows MSAA tree. `cu-abi 1.0`, the 12/10/8/0 matrix, the
four-backend type-check and the G boundary are unchanged.

## Decision

The fourth W0 pass (`prd/PRD_01_17_frontier_radar.md`, 2026-11-06) replaced
the second-hand vtable claim with **the locally installed Windows SDK
10.0.22000.0 headers** (`UIAutomationClient.h` / `UIAutomationClient.idl`).
The gate therefore moves from *vtable order published* to *vtable order
verified locally, real-desktop probe still required*. The second pass's slots
were correct; the pass-4 verification also found a new adapter trap.

0.0.15 is **probe-first**, not feature-first.

### Exact COM surface (W0 pass 4, grade O — installed SDK)

```text
CLSID_CUIAutomation   ff48dba4-60ef-4201-aa87-54103eef594e   <- instantiate this
CLSID_CUIAutomation8  e22ad333-b25f-460c-83d0-0581107395c9   <- NOT used: different proxy behaviour
IID_IUIAutomation     30cbe57d-d9d0-452a-ab13-7ac5ac4825ee
IID_IUIAutomationElement       d22108aa-8ac5-49a5-837b-37bbb3d7591e
IID_IUIAutomationElementArray  14314595-b4bc-4055-95f2-58f2e42c9855

IUIAutomationVtbl (slot, 0-based, including the 3 IUnknown slots)
   0 QueryInterface        1 AddRef             2 Release
   3 CompareElements       4 CompareRuntimeIds  5 GetRootElement
   6 ElementFromHandle     7 ElementFromPoint   8 GetFocusedElement
   9-12  four *BuildCache variants
  13 CreateTreeWalker     14-16 three view walkers
  17-19 three view conditions
  20 CreateCacheRequest
  21 CreateTrueCondition  22 CreateFalseCondition  23 CreatePropertyCondition

IUIAutomationElementVtbl
   3 SetFocus   4 GetRuntimeId   5 FindFirst   6 FindAll
  20 get_CurrentProcessId       21 get_CurrentControlType
  22 get_CurrentLocalizedControlType          23 get_CurrentName
  43 get_CurrentBoundingRectangle

IUIAutomationElementArrayVtbl
   3 get_Length   4 GetElement

TreeScope_Subtree = 0x1 | 0x2 | 0x4 = 7       UIA_HWND = void *
```

`ElementFromHandle` takes `UIA_HWND` (a pointer-sized handle, not a `u32`).
`CreateTrueCondition` returns an `IUIAutomationCondition *` passed straight
to `FindAll` — no condition vtable call is needed. `FindAll(scope, condition,
&array)` returns `IUIAutomationElementArray *`; `get_Length` / `GetElement`
enumerate it. Every COM pointer is released through its `Release` slot.

**`RECT` conversion (new trap).** `get_CurrentBoundingRectangle` writes a
Win32 `RECT` of four `LONG` values **left / top / right / bottom**, while the
shipped `a11y_controls[].rect` is `[x, y, width, height]`. The adapter must
compute `x = left`, `y = top`, `width = right - left`,
`height = bottom - top`. Passing the raw four ints through would silently
corrupt every rectangle, and the independent reader would still see a
well-typed 4-int array. A zero-width or zero-height result is a real empty
box only when the HRESULT succeeded; otherwise it is a read failure.

**HRESULT pass/fail rule.** Every COM call prints its raw `HRESULT` before
any value is interpreted. `S_OK` (`0`) is the only success; `S_FALSE` is not
a valid substitute for a read; a negative `HRESULT` is a failure with the
failing slot named. A `BSTR` from `get_CurrentName` is freed with
`SysFreeString`; a `VARIANT` is the Win64 24-byte layout already proven by
the 0.0.11 MSAA tree (the earlier 16-byte attempt crashed). Bounds are
**physical pixels** and must be reconciled with the `v0.0.10`
`display_scales[]` basis before being published.

**Grade boundary.** A header is authoritative for the SDK it ships with, not
for the runtime's provider behaviour. This is why the probe is still
mandatory: the slots are known, the provider is not.

1. W1 — a small Rust example `examples/cu_probe_uia.rs` (no `.py`, no new
   crate) creates `CLSID_CUIAutomation` via `CoCreateInstance` with
   `IID_IUIAutomation`, calls `ElementFromHandle` (slot 6) on a live `hwnd`,
   `CreateTrueCondition` (slot 21) and `FindAll` (element slot 6), then reads
   `get_CurrentControlType` (21), `get_CurrentName` (23) and
   `get_CurrentBoundingRectangle` (43) over a bounded set of elements. It
   prints raw HRESULTs, the array length and the converted `[x,y,w,h]`.
   It is a diagnostic, not a shipped capability. **Delivered — see the W1
   delivery record below.**
2. W2 — **not started**; the downgrade rule fired on the W1 verdict. What it
   would have done is recorded below as an unshipped contract, kept for the
   history of the decision.
3. W3/W4 — **not started**; no capability means no version bump or Release.

### Downgrade rule (mandatory)

If any HRESULT fails, a provider hangs, `FindAll` returns an array that
cannot be enumerated, or a zero rectangle cannot be distinguished from a
read failure, **stop and hand over the diagnosis**: publish the raw
HRESULTs, name the failing slot and switch to the next W0 candidate
(three-OS AT-SPI / AX read-only trees, or incremental a11y change
subscription). Do not guess a vtable slot, do not ship a half-working UIA
route and do not weaken the independent reader to make a journey green.
This is the same probe-first shape that de-risked the 0.0.11 MSAA tree.
**This rule fired on 2026-11-06** (W0 pass 5): the probe returned `S_OK`
everywhere but all rectangles were empty for the target WorkBuddy window, so
the zero-rectangle case could not be distinguished from a withheld read at
the target that matters. W2 therefore stops.

### Reader contract (unshipped contract, recorded for the decision history)

The following was written down before W1 to satisfy checklist item 3. Since
W2 did not start, none of it ships and no reader assertion was changed or
weakened:

- W2 would have added `a11y_source` values (`uia` / `msaa-fallback`) and a
  named `a11y_reason` on every failed or refused UIA scan.
- Every `a11y_controls[]` row keeps the 0.0.14 schema: `name`, `role`,
  `role_name`, `rect` as exactly four ints `[x,y,w,h]`, `depth`, `child_id`,
  `value`, `state`, `state_names`. The reader recomputes `state_names` from
  the published integer bit mask and rejects a guessed set; it also rejects
  a `rect` whose width/height are negative, which is the signature of a
  raw `RECT` leaking through unconverted.
- Every 0.0.6-0.0.14 reader assertion stays. A UIA row is never allowed to
  weaken a grant, scale, wait, handle-focus or MSAA assertion.

### Three-OS behaviour (unshipped contract)

Had W2 started: Windows would have run the probe and published UIA rows,
otherwise MSAA rows with a named reason; macOS / Linux / fallback would have
kept `a11y_controls: []` with their existing named reasons and contributed
no UIA step. The four-backend type gate would have typed all four
configurations in the same commit, with no new native signature, crate or
link-time platform library. **Only the W1 probe ran; the four-backend gate
was still executed and passed.**

### Plan-completeness checklist (`PRD_01_23`) — 6/6

| # | Requirement | Where it is written down | State |
|---|---|---|---|
| 1 | exact COM surface: CLSID/IID, slots, `RECT`, HRESULT rule | "Exact COM surface" above, from the installed SDK | `[v]` plan |
| 2 | downgrade branch: hang, zero rect, missing pattern | "Downgrade rule" + HRESULT pass/fail rule | `[v]` plan |
| 3 | reader contract: additive rows, retained assertions, rejection rules | "Reader contract" above | `[v]` plan |
| 4 | three-OS behaviour | "Three-OS behaviour" above | `[v]` plan |
| 5 | non-goals and G boundary restated | "Non-goals" below | `[v]` plan |
| 6 | W1 becomes eligible only after 1-5 | this table | `[v]` plan |

**All six items were written down before W1 started** (W0 pass 4). This
checklist passing was a **plan** fact, not a shipped capability; W1 then
produced the W0 pass 5 verdict recorded under "W1 delivery record" below.

## Work breakdown and gates

```text
0.0.15 — UIA probe verdict
├── [v] W1 — Rust UIA diagnostic probe, run on a real Windows desktop
│   invariant held: no shipped source change, raw HRESULTs printed for
│   every call, four cfgs type-check, no new crate, no new platform lib
├── [no] W2 — UIA-first + MSAA fallback: STOPPED by the downgrade rule
│   reason: the target WorkBuddy window exposes 8 nodes, 0 non-Pane
│   controls and 1 name, with a cold-start window where every RECT is zero;
│   the shipped MSAA tree reads real controls on that same window, so a
│   UIA-first row would regress the L3 target
├── [no] W3 / W4 — no docs-truth bump and no Release without a capability
└── [_] next W0 candidate — AT-SPI / AX trees or incremental a11y events
```

## W1 delivery record (2026-11-06, real desktop, grade X)

`examples/cu_probe_uia.rs` is a read-only diagnostic example. It asks the
repo's own `CuHost::invoke_json({"op":"observe"})` for a live `hwnd`, then
calls `CoCreateInstance` -> `ElementFromHandle` (slot 6) ->
`CreateTrueCondition` (slot 21) -> `FindAll` (element slot 6) ->
`get_Length` (array slot 3) -> `GetElement` (array slot 4) ->
`get_CurrentControlType` (21) / `get_CurrentName` (23) /
`get_CurrentBoundingRectangle` (43), printing every raw HRESULT.

**Verification gates:** `cargo check --locked --example cu_probe_uia` clean,
`cargo test --locked` 119 passed / 3 ignored, `cu-cfg-check.py` four
configurations OK, `rustfmt --check` clean on the new file. No `src/` file,
version source, `cu-abi`, capability row or matrix cell changed.

| Handle | Window | Elements | Non-Pane controls | Non-zero rects | Verdict |
|---|---|---|---|---|---|
| desktop root | control via `GetRootElement` slot 5 | 1 | — | 1 (`0,0,1680,1050`) | proves slot 43 + `RECT` layout |
| 33689578 | `WorkBuddy` (Electron, the L3 target), cold start | 8 | 0 / 8 | 0 / 8 | `PROBE_PARTIAL` — all `RECT` zero |
| 33689578 | same handle, four consecutive re-runs | 8 | **0 / 8** | 8 / 8 (`840,0,1680,1014`) | `PROBE_OK` numerically, but no addressable control |
| 59771304 | second `WorkBuddy` window | 8 | 0 / 8 | 0 / 8 | `PROBE_PARTIAL` |
| 269226 | Brave | 399 | 8 / 16 | 10 / 16 | `PROBE_OK` |
| 2496684 | Electron-style MiniCon window | 639 | — | 14 / 16 | `PROBE_OK` |
| 66886 | Sourcetree | 438 | 16 / 16 | 16 / 16 | `PROBE_OK` |
| 15339458 | GVIM | 22 | 15 / 16 | 15 / 16 | `PROBE_OK` |
| 12390440 / 21564726 | `cmd.exe` | 8 | 8 / 8 | 8 / 8 | `PROBE_OK` |

`UIA_PaneControlTypeId` is 50033; the probe counts any other control type as a
potentially addressable control.

**Interpretation.** The COM surface is correct: every slot returned `S_OK`,
the desktop root read a usable rectangle, and five unrelated applications
produced hundreds of named, bounded, non-Pane nodes. Two distinct facts about
the **target WorkBuddy provider** then decided the gate:

1. **No addressable controls.** Both WorkBuddy windows expose exactly 8 nodes,
   all `control_type=50033` (Pane), only one named. Zero non-Pane controls
   means no input box, button or menu to click or type into — while the
   shipped 0.0.11 MSAA tree reads real controls on that same client. A
   UIA-first route would therefore *regress* `observe.a11y` for L3.
2. **A cold-start zero-rect window.** The first reads of the target returned
   `S_OK` with all eight `RECT`s zero; four consecutive re-runs of the same
   handle then returned `840,0,1680,1014` stably. This is the documented
   provider behaviour in `PRD_01_17` (unrealised / virtualised rows, window
   or integrity context), and it is a second, independent reason not to ship:
   a scan that silently flips from empty to non-empty without a generation
   change cannot satisfy the independent reader's "no guessed tree" rule.

Neither fact is a wrong vtable slot. Shipping UIA-first plus an MSAA fallback
would still add a large FFI surface whose first row on the L3 target is
either empty or non-addressable.

**Decision (controller, `PRD_01_24` D2.5):** apply the mandatory downgrade
rule on the cold-start zero rectangles, and decline UIA-first on the separate
finding of zero addressable controls. W2 does not start, no UIA capability
is claimed, no version bump or Release chain runs. The probe stays in the
tree as reproducible X-grade evidence; the next W0 pass examines the AT-SPI /
AX and incremental-event candidates. `v0.0.14` remains the last shipped
version. No owner choice was requested.

W0 pass 6 then explained the empty tree as Chromium's lazy `AXMode`
activation (`--force-renderer-accessibility`, an assistive technology, or
Electron's `setAccessibilitySupportEnabled`) — a **target-app launch
precondition outside `screen`**, not a CU reader bug. It also recorded the
Linux session-bus / Wayland and macOS dual-TCC conditions for the AT-SPI / AX
candidate. It sharpens the reopen trigger in `PRD_01_24` and does **not**
reopen W2; no implementation starts from this pass.

## Non-goals

- `[no]` any UIA write, invoke pattern, focus setting, event subscription or
  control-pattern dispatch in this version; the tree stays read-only.
- `[no]` an image arm, batched actions, app-level effect evidence, safety
  policy or a model loop.
- `[no]` requesting or changing an OS grant, display mode or DPI setting.
- `[no]` widening `screen` into host control; the G boundary in `PRD_01_08`
  is unchanged. G direction (a) is a **decided** controller decision
  (`PRD_01_24`); read-only telemetry has four explicit reopen triggers and
  real resource control is `[no]`.
- `[no]` re-publishing or overwriting any shipped v0.0.4-v0.0.14 asset.
