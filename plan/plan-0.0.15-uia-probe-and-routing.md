# 0.0.15 — UIA probe first, then mixed structured routing (plan complete 6/6, W1 eligible)

Status: `[_]` **plan complete 6/6, W1 eligible, not started**, after `v0.0.14`
was published as a public signed Release and accepted from the owner's view
on 2026-11-06.
Under the owner's 2026-11-06 evening direction, this phase is study and
structure: **no W1/W2 code, no test, no version bump and no Release chain
until the plan is complete** (三思后行 / 分而治之). W0 pass 4 completed the
plan on 2026-11-06: the COM surface is verified against the installed
Windows SDK, and the six-item completeness checklist below is 6/6. **W1 is
eligible, not started**; the controller starts it in its own cadence
(`PRD_01_24`), with no owner choice required.
**No source, version or Release change belongs to this document.** The source
tree stays at `0.0.14` until this version's W4 runs.

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
   one `get_CurrentName` (slot 23) and one `get_CurrentBoundingRectangle`
   (slot 43). It prints raw HRESULTs, the array length and the converted
   `[x,y,w,h]`. It is a diagnostic, not a shipped capability.
2. W2 — only if the probe succeeds on a real Windows desktop, `native::a11y_controls`
   gains a UIA-first route with the existing MSAA walk as fallback, publishing
   additive `a11y_source` values. The normalised 0.0.14 node schema is the
   translation target; no new capability row, no ABI bump, no OS grant.
3. W3/W4 — docs, version bump and the public signed Release chain as usual.

### Downgrade rule (mandatory)

If any HRESULT fails, a provider hangs, `FindAll` returns an array that
cannot be enumerated, or a zero rectangle cannot be distinguished from a
read failure, **stop and hand over the diagnosis**: publish the raw
HRESULTs, name the failing slot and switch to the next W0 candidate
(three-OS AT-SPI / AX read-only trees, or incremental a11y change
subscription). Do not guess a vtable slot, do not ship a half-working UIA
route and do not weaken the independent reader to make a journey green.
This is the same probe-first shape that de-risked the 0.0.11 MSAA tree.

### Reader contract (additive, nothing weakened)

- W2 adds `a11y_source` values (`uia` / `msaa-fallback`) and a named
  `a11y_reason` on every failed or refused UIA scan.
- Every `a11y_controls[]` row keeps the 0.0.14 schema: `name`, `role`,
  `role_name`, `rect` as exactly four ints `[x,y,w,h]`, `depth`, `child_id`,
  `value`, `state`, `state_names`. The reader recomputes `state_names` from
  the published integer bit mask and rejects a guessed set; it also rejects
  a `rect` whose width/height are negative, which is the signature of a
  raw `RECT` leaking through unconverted.
- Every 0.0.6-0.0.14 reader assertion stays. A UIA row is never allowed to
  weaken a grant, scale, wait, handle-focus or MSAA assertion.

### Three-OS behaviour

- Windows: runs the probe; on success publishes UIA rows, otherwise MSAA
  rows with a named reason. macOS / Linux / fallback keep `a11y_controls: []`
  with their existing named reasons and contribute no UIA step.
- The four-backend type gate types all four configurations in the same
  commit; no new native signature, crate or link-time platform library.

### Plan-completeness checklist (`PRD_01_23`) — 6/6

| # | Requirement | Where it is written down | State |
|---|---|---|---|
| 1 | exact COM surface: CLSID/IID, slots, `RECT`, HRESULT rule | "Exact COM surface" above, from the installed SDK | `[v]` plan |
| 2 | downgrade branch: hang, zero rect, missing pattern | "Downgrade rule" + HRESULT pass/fail rule | `[v]` plan |
| 3 | reader contract: additive rows, retained assertions, rejection rules | "Reader contract" above | `[v]` plan |
| 4 | three-OS behaviour | "Three-OS behaviour" above | `[v]` plan |
| 5 | non-goals and G boundary restated | "Non-goals" below | `[v]` plan |
| 6 | W1 becomes eligible only after 1-5 | this table | `[v]` plan |

**All six items are now written down.** W1 is eligible. The controller
starts it in its own cadence; no owner choice is required (`PRD_01_24`).
This checklist passing is a **plan** fact, not a shipped capability: no
source, test, version or Release change belongs to this document.

## Work breakdown and gates

```text
0.0.15 — UIA probe + mixed routing
├── [_] W1 — Rust UIA diagnostic probe
│   invariant: no source change to the shipped path, raw HRESULTs printed,
│   four cfgs type-check, no new crate, no link-time platform library
├── [_] W2 — UIA-first + MSAA fallback (only if W1 passes)
│   invariant: cu-abi 1.0, 11 capability rows, 12/10/8/0 matrix unchanged,
│   reader recomputes every row under the 0.0.14 schema, named reason on
│   every UIA failure, MSAA fallback never silently masks a refused scan
├── [_] W3 — the truth of the docs (radar, PRD, README, release notes)
└── [_] W4 — version bump and public signed Release
```

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
