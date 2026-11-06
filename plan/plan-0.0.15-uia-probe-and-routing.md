# 0.0.15 — UIA probe first, then mixed structured routing (planning, not started)

Status: `[_]` **PLANNING — not started**, after `v0.0.14` was published as a
public signed Release and accepted from the owner's view on 2026-11-06.
Under the owner's 2026-11-06 evening direction, this phase is study and
structure: **no W1/W2 code, no test, no version bump and no Release chain
until the plan is complete** (三思后行 / 分而治之). The plan-completeness
checklist in `prd/PRD_01_23_capability_maturity_subtree.md` must pass first.
**No source, version or Release change belongs to this document.** The source
tree stays at `0.0.14` until this version's W4 runs.

Baseline: `v0.0.14` published the normalised a11y node schema on top of the
0.0.11 Windows MSAA tree. `cu-abi 1.0`, the 12/10/8/0 matrix, the
four-backend type-check and the G boundary are unchanged.

## Decision

The second W0 pass (`prd/PRD_01_17_frontier_radar.md`, 2026-11-06) moved the
UIA gate from *vtable unknown* to *vtable order published, real-desktop probe
still required*. The `IUIAutomation` vtable order is documented (Go
`w32uiautomation` binding): `ElementFromHandle` is slot 6,
`CreateTrueCondition` slot 21, and `IUIAutomationElement` exposes `FindAll`,
`Get_CurrentName`, `Get_CurrentBoundingRectangle`, `Get_CurrentControlType`
and cached equivalents. Three documented traps matter: the bounding
rectangle is a four-int `RECT`, not a four-double `UiaRect`; swallowing a
failed HRESULT into a zero rectangle hides a read failure; UIA bounds are
physical pixels and must be reconciled with the `v0.0.10` display-scale
basis.

0.0.15 is therefore **probe-first**, not feature-first:

1. W1 — a small Rust example `examples/cu_probe_uia.rs` (no `.py`, no new
   crate) creates `CLSID_CUIAutomation` via `CoCreateInstance`, calls
   `ElementFromHandle` on a live `hwnd`, then `CreateTrueCondition` and
   `FindAll`, reads one name and one `RECT`, and prints raw HRESULTs plus the
   element count. It is a diagnostic, not a shipped capability.
2. W2 — only if the probe succeeds on a real Windows desktop, `native::a11y_controls`
   gains a UIA-first route with the existing MSAA walk as fallback, publishing
   additive `a11y_source` values. The normalised 0.0.14 node schema is the
   translation target; no new capability row, no ABI bump, no OS grant.
3. W3/W4 — docs, version bump and the public signed Release chain as usual.

### Downgrade rule (mandatory)

If the probe fails, hangs or returns a zero rectangle that cannot be
distinguished from a read failure, **stop and hand over the diagnosis**:
publish the raw HRESULTs, name the failing slot and switch to the next W0
candidate (three-OS AT-SPI / AX read-only trees, or incremental a11y change
subscription). Do not guess a vtable slot, do not ship a half-working UIA
route and do not weaken the independent reader to make a journey green.
This is the same probe-first shape that de-risked the 0.0.11 MSAA tree.

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
  is unchanged. Resource-domain telemetry / control stays an open owner
  decision recorded in `PRD_01_17`.
- `[no]` re-publishing or overwriting any shipped v0.0.4-v0.0.14 asset.
