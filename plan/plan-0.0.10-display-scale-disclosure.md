# 0.0.10 — display scale / coordinate-basis disclosure

Status: `[v]` **W1, W2 and W3 delivered**; `[_]` W4 (the version bump and the
public signed Release) is the only open work in this plan. W1/W2 landed in
`040d50d`; the compile gate `42177115600` and the three-OS journeys
`42177115725` are both green, and the controller's independent reader
recomputed all three downloaded `cu-journey.json` artifacts as PASS (Linux
27 performed / `scale=unknown` with a reason, macOS 24 / `scale=1`, Windows
30 / `scale=1`; `disclose_scales` is required on every OS). The source tree
still reads `0.0.9` until W4 runs.

Baseline: `v0.0.9` widened `op=disclose` with read-only `environment` and
typed `grants[]`; `cu-abi 1.0`, the 12/10/8/0 matrix and four-backend
type-check are unchanged. Four consecutive versions (0.0.6-0.0.9) delivered
desktop evidence without declaring a new capability row.

## Decision

0.0.10 publishes the **coordinate basis**: the scale between the logical
coordinates a model reads from a screenshot and the physical coordinates a
pointer event needs. It is pure observation, no new native effect, no new
grant, no policy engine and no loop decision.

The reason is to publish the coordinate basis, not a measured DPI failure.
An earlier L1 trial note claimed a ~1.25x mismatch between the screenshot and
the desktop; that was a controller misreading. CU self-reports its
`virtual_desktop`, `desktop` and PNG geometry consistently at 1680x1050 with
a self-consistent coordinate system, and a click taken from CU's own image hit
the target on the first try. Three-OS CI measures `scale` as `1` / `1` /
`unknown` (macOS / Windows / Linux); this host has no 1.25x DPI scaling. The
earlier missed clicks came from mixing UIA, PIL and CU coordinate systems and
from focus never reaching the input box, not from a display scale. Publishing
this basis still comes first: it gives A5 (the model-visible image arm) and
any cross-DPI / multi-monitor / scaled session a declared ratio instead of a
caller's guess. Scale disclosure therefore precedes A5.

| Deferred, with the reason recorded | Unlock condition |
|---|---|
| A5 / D2 model-visible image arm | separate PRD + PNG budget/retention decision |
| B7b `wait` + D4 ceilings | loop/budget decision, not a desktop verb; would add a 13th capability row. Hard evidence from the L1 trial, not a model-capability gap: an `observe` issued immediately after an `act` captured the pre-reflow Chromium frame, and `type` plus its send click had to be split into separate runs before the new state was visible |
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
0.0.10 — display scale / coordinate-basis disclosure (implemented;
          public signed Release pending)
├── [v] W1 — the read-only scale probe contract
│   files: src/cu.rs, src/wire.rs, src/screen.rs, src/screen_linux.rs,
│          src/screen_macos.rs, scripts/cu-cfg-check.py
│   invariant: cu-abi 1.0, 12-row matrix unchanged, four cfgs type-check,
│   cargo test --locked green, no new capability row
│   evidence: commit 040d50d; compile gate 42177115600
├── [v] W2 — a real disclosure journey in CI
│   files: examples/cu_journey.rs, scripts/cu-journey-verify.py,
│          .github/workflows/cu-journeys.yml
│   invariant: required disclose_scales on all three OSes; reader recomputes
│   every row's scale, the logical/physical relation and the reason rule
│   evidence: journeys 42177115725; three cu-journey.json artifacts
│   recomputed PASS by the controller's independent reader
├── [v] W3 — the truth of the docs
│   files: PRD.md, README.md, PRD_01_03, PRD_01_08, PRD_01_12,
│          PRD_01_13, this plan, release notes
│   invariant: E5a marked delivered, A5/B7b/A4/F3/C5/C6/B8 still deferred;
│   the 1.25x DPI story corrected; no [no] weakened
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

## Delivery record

| W | Evidence |
|---|---|
| W1/W2 | commit `040d50d`; compile gate `42177115600` and three-OS journeys `42177115725`; the three `cu-journey.json` artifacts (Linux / macOS / Windows) recomputed PASS by the controller's independent reader |
| W3 | this plan, `PRD.md`, `README.md`, `PRD_01_03`, `PRD_01_08`, `PRD_01_12`, `PRD_01_13` and the release notes, on that three-OS CI evidence |
| W4 | release chain run IDs pending; public signed Release pending |

The W4 row is filled in by the release chain. No shipped `v0.0.4`-`v0.0.9`
asset is re-published or overwritten.

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
