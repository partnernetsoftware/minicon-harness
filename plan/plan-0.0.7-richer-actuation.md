# 0.0.7 — richer actuation: a real path drag and two-axis scroll

Status: `[_]` plan + contract delivered (2026-11-05), **no source, version or
Release change**. Written after `v0.0.6` was published, as the owner's
"plan the next version after each public release" loop requires. The version
still in the source tree is `0.0.6` until W4 of this plan runs.

Baseline: `v0.0.6` is a public company-signed Latest Release; interaction
closure is green on three OSes, every performed button event carries an
actuation receipt, and the capability matrix is windows 12/12, linux 10/12,
macos 8/12, other 0/12. `cu-abi 1.0` is frozen.

## Decision: what 0.0.7 is, and what it deliberately is not

`PRD.md`'s 0.7.3 chapter ("richer actuation", B6/B7/B8) has an unlock
condition: 0.0.6 interaction closure green on three OSes. That condition is
now satisfied. The owner's direction for this loop is **concentrate on
computer use, do not grow harness capabilities**, so 0.0.7 takes the two
parts of B6/B7 that are pure desktop verbs and need no new OS grant, no new
fixture and no model-loop change:

| Delivered in 0.0.7 | Leaf | Shape |
|---|---|---|
| multi-point drag path | B6 (half) | `act:drag` accepts a bounded `path[]`; the backend interpolates each segment instead of one straight line |
| two-axis scroll | B7 (half) | `act:scroll` accepts bounded `delta_x` / `delta_y`; Windows HWHEEL, macOS two-axis CG event, Linux buttons 6/7 |

| Deferred, with the reason recorded so it is not re-litigated | Unlock condition |
|---|---|
| held-key modifiers (B6 half) | a keyboard state machine on all three backends with a guaranteed release path when the foreground is lost, plus its own receipt semantics. A modifier is not a button event; folding it into the 0.0.6 receipt would blur what the receipt asserts |
| explicit bounded `wait` (B7 half) | a loop/budget decision (D4), not a desktop verb. It would add a 13th capability row and change every reader's `EXPECTED`. The owner explicitly deferred harness-side capability work |
| batched `actions[]` (B8) | its own admission/stale design: one call with several effects conflicts with "one act -> one verdict -> one receipt" and with `as_of` pinning |
| image arm / a11y trees / safety checks / app-level effect evidence (0.7.1, 0.7.2, 0.7.4, 0.7.5, 0.7.6) | unchanged unlock conditions in `PRD.md`; none is met by a hosted runner today |

Consequence: **the capability matrix still does not grow**. This is the same
defensible boundary as 0.0.6: strengthen the evidence and the expressiveness
of declared verbs before declaring new ones. Both new shapes are additive wire
parameters, not new capability names. `cu-abi` stays 1.0.

## Contract owner: `prd/PRD_01_09_richer_actuation.md`

- `act:drag`: optional `path: [[x,y], ...]`, at most 16 points, every point
  inside the virtual desktop (negative coordinates allowed). Omitting it keeps
  today's single `to_x/to_y` behaviour bit-for-bit. The final pointer readback
  must land at the **last** point; the actuation receipt stays `downs=1,ups=1`
  and gains an additive `path_points` count.
- `act:scroll`: `delta_x` / `delta_y` replace the single `delta` (which stays
  accepted as a vertical alias so 0.0.6 callers and the journey keep working).
  Both axes are bounded; `(0,0)` is a named refusal, never a silent no-op.
  The readback keeps its `scrolled ... at X,Y` shape so the independent
  reader's coordinate assertion is unchanged; the two axes are published as
  structured evidence.
- One function set, four configurations: `native::drag` and `native::scroll`
  change signature on Windows, macOS, Linux and the fallback backend together;
  `scripts/cu-cfg-check.py` type-checks all configurations.
- A point outside the virtual desktop is refused with the geometry named,
  never clipped by the OS. A path with one point is refused, not treated as a
  click.

## Work breakdown and gates

```text
0.0.7 — richer actuation
├── [_] W1 — path drag and two-axis scroll in the contract
│   files: src/cu.rs, src/wire.rs, src/screen.rs, src/screen_macos.rs,
│          src/screen_linux.rs, scripts/cu-cfg-check.py
│   invariant: cu-abi 1.0, 12-row matrix unchanged, all four cfgs
│   type-check, cargo test --locked green on all three CI jobs
│   evidence: cu-platforms.yml run, matrix test still 12/10/8/0
├── [_] W2 — real richer-actuation journeys in CI
│   files: examples/cu_journey.rs, scripts/cu-journey-verify.py,
│          .github/workflows/cu-journeys.yml
│   invariant: new required steps act_drag_path, act_scroll_x, act_scroll_y
│   on all three OSes; every performed step keeps pointer readback and (for
│   drag) a matching actuation receipt; final observe strictly newer and live
│   evidence: cu-journeys.yml run, three cu-journey.json artifacts,
│   independent reader output recomputing the path's last point and both axes
├── [_] W3 — the truth of the docs
│   files: PRD.md, README.md, prd/PRD_01_03_computer_use.md,
│          prd/PRD_01_09_richer_actuation.md, release notes
│   invariant: B6/B7 split recorded (delivered half vs deferred half), the
│   0.0.6 frontier marked shipped, no stale "planned" claim for a shipped verb
└── [_] W4 — 0.0.7 version bump and public signed Release
    files: Cargo.toml, Cargo.lock, release-policy.json,
           loader/ape-version.rc, .github/release-notes.md
    invariant: four version sources in ONE commit; dry-run before publish;
    the 0.0.5/0.0.6 four release invariants must not regress
    evidence: run IDs + owner-view anonymous download and re-hash
```

### W2 step design

| New step | Request | Required on | Why it is harmless |
|---|---|---|---|
| `act_drag_path` | `action=drag, path=[[x,y],[x+40,y+20],[to_x,to_y]]` | all 3 | an empty client-area drag; the last point's readback and the receipt are checked |
| `act_scroll_x` | `action=scroll, delta_x=120, delta_y=0` | all 3 | horizontal wheel in an empty document; no navigation control is targeted |
| `act_scroll_y` | `action=scroll, delta_x=0, delta_y=-120` | all 3 | the existing vertical path with the new spelling; proves both axes are independent |

The independent reader is extended, not weakened:

- `EXPECTED` stays exactly 12 rows with windows 12 / linux 10 / macos 8 true.
- `act_drag_path`: `evidence["actuation"]["path_points"]` equals the request's
  path length, and the pointer readback lands at the **last** point within the
  4 px tolerance.
- `act_scroll_x` / `act_scroll_y`: structured evidence carries the requested
  axis pair, the other axis is zero, and the pointer readback still parses.
- Every existing 0.0.6 assertion (receipts, generations, live final frame,
  declared-but-skipped rule, BOUNDARY) stays. A `BOUNDARY` note must never
  excuse a missing receipt on a performed step.

### W4 release invariants (must not regress)

1. `publish` keeps `environment: release` with `github.token`.
2. Signed Windows cells are rewritten into both Windows zips and their
   `.sha256` sidecars are recomputed.
3. `post-publish` re-hashes the **downloaded** assets against
   `signing-receipt.json` `after_sha256`.
4. Defender uses `MpCmdRun -File` and `TAG` resolves through
   `needs: [verify, ...]`.

Chain, all `workflow_dispatch` on `main`, one shared `head_sha`:

```
minicon-harness-com.yml pack -> candidate.yml (source_sha, pack_run_id)
-> company-signing.yml (qualification_only=false) -> release.yml dry_run=true
-> release.yml dry_run=false (publish + Defender + post-publish)
```

Owner-view acceptance is the last step, not the exit code: anonymous HTTP 200
on the Release page and one asset, a fresh download of all 15 assets re-hashed
locally against the receipt and sidecars, and the downloaded APE printing
`minicon-harness 0.0.7`.

## Risks and downgrade rules

- **macOS two-axis scroll**: `CGEventCreateScrollWheelEvent2` is non-variadic
  but a fixed Rust fn pointer must never call a C variadic (the 0.0.5 arm64
  lesson). New CG symbols go through the existing `dlopen`/`dlsym` helper.
- **Linux horizontal wheel** is buttons 6/7; Xvfb `xmessage` accepts them but
  produces no visible effect. The receipt for scroll is the pointer readback
  plus structured axis evidence, **not** an application scroll claim.
- **Windows HWHEEL** uses `MOUSEEVENTF_HWHEEL` with a signed delta; the sign
  convention must be normalised in evidence, never left to the reader.
- **Path interpolation**: each segment keeps stepped intermediate motion, or
  drag-and-drop implementations see one jump. Total steps stay bounded.
- **No assertion is relaxed.** If a variant cannot be qualified on an OS it is
  a named refusal with OS and reason in the journey record; the matrix does
  not change to hide it, and the reader does not drop the check.
- **Scope creep**: no new crate, link-time platform library, tool, ABI bump,
  engine, image arm or policy engine. If W1/W2 needs one, split it into a
  separate PRD rather than widening 0.0.7.

## Non-goals

- `[no]` a11y trees (AX / AT-SPI / UIA COM), image arm in the wire codec,
  Wayland-native protocol, runtime backend selection.
- `[no]` held-key modifiers, explicit `wait`, batched `actions[]` in this
  version; each has an unlock condition above.
- `[no]` app-level state-change evidence; a self-hosted echo window is a
  separate design and gate.
- `[no]` new crate, runtime, plugin host, bun/WASM/ujs embedding; PRD 31
  readiness stays BLOCKED.
- `[no]` re-publishing or overwriting v0.0.4 / v0.0.5 / v0.0.6 assets.
- `[no]` claiming `[v]` for a verb that only compiled, or relaxing a reader
  assertion to make a journey green.

## Delivery record

Filled in as each W closes, with the CI run IDs, the three journey artifacts,
the release chain runs and the owner-view anonymous acceptance result. Until
then every W above stays `[_]` and the source version stays `0.0.6`.
