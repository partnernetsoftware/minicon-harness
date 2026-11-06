# 0.0.10 — display scale / coordinate-basis disclosure

Status: `[_]` planned after `v0.0.9` was published as a public signed Release
and accepted from the owner's view. **No source, version or Release change
belongs to this document.** The source tree stays at `0.0.9` until W4 runs.

Baseline: `v0.0.9` widened `op=disclose` with read-only `environment` and
typed `grants[]`; `cu-abi 1.0`, the 12/10/8/0 matrix and four-backend
type-check are unchanged. Four consecutive versions (0.0.6-0.0.9) delivered
desktop evidence without declaring a new capability row.

## Decision

0.0.10 publishes the **coordinate basis**: the scale between the logical
coordinates a model reads from a screenshot and the physical coordinates a
pointer event needs. It is pure observation, no new native effect, no new
grant, no policy engine and no loop decision.

The reason is a measured failure, not a guess. In the L1 self-hosted
orchestration trial the controller drove the WorkBuddy client GUI itself:
every click derived from screenshot coordinates missed the input box, while a
UIA `EditControl` rectangle hit it immediately. The screenshot and the desktop
differed by about 1.25x. `PRD_01_12` puts A5 (the model-visible image arm)
first in the L2 queue, but an image arm without a scale basis would reproduce
this failure with more bytes. Scale disclosure therefore precedes A5.

| Deferred, with the reason recorded | Unlock condition |
|---|---|
| A5 / D2 model-visible image arm | separate PRD + PNG budget/retention decision |
| B7b `wait` + D4 ceilings | loop/budget decision, not a desktop verb; would add a 13th capability row |
| A4 accessibility trees | OS grants + three-OS fixtures |
| F3 app-level effect evidence / held-key effect evidence | side-effect-free echo fixture on three OSes |
| C5 / C6 safety + hand-off, B8 batched actions | policy PRD / admission+stale design |

The capability matrix still does not grow: scale is an additive evidence field,
not a new capability name. `cu-abi` stays 1.0.

## Contract owner: `prd/PRD_01_13_display_scale_disclosure.md`

- `disclose` gains `evidence["environment"]["display_scales"]`: one row per
  display with `display_id`-free index, `logical_rect`, `physical_rect` and
  `scale`; `observe.displays[]` gains the same `scale` per row.
- `scale` is one of a typed bounded set; a cell that cannot answer safely
  publishes `unknown` with a named reason, never a plausible 1.0.
- Read-only only: `GetDpiForWindow` / `GetDpiForSystem` /
  `GetSystemMetricsForDpi` on Windows, `backingScaleFactor` on macOS,
  `Xft.dpi` + `XRRScreenResources` read-only query on Linux. No scaling change,
  no display-mode change, no DPI-awareness mutation and no permission prompt.
- One function set, four configurations: `native::display_scales()` changes
  together on Windows, macOS, Linux and fallback; `cu-cfg-check.py` types all
  four.
- Every 0.0.6-0.0.9 reader assertion stays. The new reader rule recomputes
  `physical_rect == logical_rect * scale` and rejects a missing/unknown row
  without a reason.

## Work breakdown and gates

```text
0.0.10 — display scale / coordinate-basis disclosure
├── [_] W1 — the read-only scale probe contract
│   files: src/cu.rs, src/wire.rs, src/screen.rs, src/screen_linux.rs,
│          src/screen_macos.rs, scripts/cu-cfg-check.py
│   invariant: cu-abi 1.0, 12-row matrix unchanged, four cfgs type-check,
│   cargo test --locked green, no new capability row
├── [_] W2 — a real disclosure journey in CI
│   files: examples/cu_journey.rs, scripts/cu-journey-verify.py,
│          .github/workflows/cu-journeys.yml
│   invariant: required disclose_scales on all three OSes; reader recomputes
│   every row's scale, the logical/physical relation and the reason rule
├── [_] W3 — the truth of the docs
│   files: PRD.md, README.md, PRD_01_03, PRD_01_08, PRD_01_13, release notes
│   invariant: E5a marked delivered, A5/B7b/A4/F3/C5/B8 still deferred
└── [_] W4 — version bump and public signed Release
    files: Cargo.toml, Cargo.lock, release-policy.json,
           loader/ape-version.rc, .github/release-notes.md
    invariant: four version sources in ONE commit; dry-run before publish;
    the four release invariants must not regress
```

The controller owns W3/W4 planning, the three-OS CI gate, the independent
journey recompute, the release gate and the owner-view acceptance. **The
implementation work is dispatched to a sub-session through the client GUI**,
with a per-stage progress file; the controller only polls, verifies and
decides. This is the L1 division that `PRD_01_12` records.

## Risks and downgrade rules

- **A scale that is not actually 1.0 on CI.** Headless runners often report
  1.0; that is a real answer, not a failure. A cell that cannot probe must
  say `unknown` with a reason, and the reader must not convert it into a pass.
- **No reader assertion is weakened.** If a probe cannot be qualified on an
  OS it stays a named `unknown` in the journey record.
- **Scope creep**: no image arm, no a11y tree, no new crate, no link-time
  platform library, no ABI bump, no policy engine. If W1/W2 needs one, split
  it into a separate PRD.

## Non-goals

- `[no]` image arm, explicit `wait`, batched actions, a11y trees, app-level
  effect evidence and safety policy in this version.
- `[no]` requesting or changing a display mode, DPI setting or OS grant.
- `[no]` widening `screen` into host control; the G boundary in `PRD_01_08`
  is unchanged.
- `[no]` re-publishing or overwriting any shipped v0.0.4-v0.0.9 asset.
