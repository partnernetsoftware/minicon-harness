# 0.0.13 — bounded observation wait (public signed Release accepted)

Status: `[v]` W1-W4 delivered in `40276ae` (W1/W2) and `59e6fd2` (W3/W4),
after `v0.0.12` was published as a public signed Release and accepted from
the owner's view on 2026-11-06. Three-OS CI `42213269581` / `42213269486`
and `42214274457` / `42214326088` are success. The source tree stayed at
`0.0.12` until W3.

Baseline: `v0.0.12` published handle-addressed window targeting
(`act:focus` by `hwnd`, `target_hwnd` readback) with three-OS CI evidence and
an accepted public signed Release. `cu-abi 1.0`, the 12/10/8/0 matrix and the
four-backend type-check are unchanged. Seven consecutive versions
(0.0.6-0.0.12) delivered desktop evidence without declaring a new capability
row.

## Decision

0.0.13 closes the measured **act/observation timing gap**: `op=observe`
gains an optional `wait_ms` that sleeps once for a bounded interval before
taking a fresh snapshot, then publishes `evidence["wait"]` with the
requested, elapsed and cumulative milliseconds, the call count and the new
generation. It is an additive observe parameter, not a new verb, not a model
loop and not a new capability row.

The reason is hard L1 evidence, not a guess. In the self-hosted orchestration
trial an `observe` issued immediately after an `act` captured a pre-reflow
Chromium frame, and `type` plus its send click had to be split into separate
runs before the new state was visible. A coordinate basis (0.0.10), a
structured Windows a11y tree (0.0.11) and an unambiguous window handle
(0.0.12) do not fix stale timing. D4 ceilings are part of this version: a
per-call bound, a cumulative per-host bound and a call-count bound, each a
named refusal before any sleep.

| Deferred, with the reason recorded | Unlock condition |
|---|---|
| A5 / D2 model-visible image arm | separate PRD + PNG budget/retention decision; the owner rejected screenshot-and-vision coordinate reads as slow and token-heavy |
| B8 batched actions | admission/stale design |
| AT-SPI (Linux) / AX (macOS) trees | OS grants + three-OS fixtures; the bounded Windows MSAA subset shipped in 0.0.11 |
| F3 app-level effect evidence | side-effect-free echo fixture on three OSes |
| C5 / C6 safety + hand-off | policy PRD first |
| L2 self-sustained loop | A5 / D2, B7b + D4 and A4 / equivalent all have their own gates; this version closes only B7b + D4 |

The capability matrix does not grow: `wait_ms` is an additive parameter and
`evidence["wait"]` is additive evidence. `cu-abi` stays 1.0 and the six false
cells are untouched.

## Contract owner: `prd/PRD_01_16_bounded_wait.md`

- `observe { op:"observe", wait_ms: 250 }` sleeps once before the snapshot and
  publishes `evidence["wait"]`: `requested_ms`, `elapsed_ms`,
  `cumulative_ms`, `wait_calls`, `generation` and a named reason only on a
  ceiling refusal. An ordinary observe without `wait_ms` is unchanged.
- Bounds: `1..=2000` ms per call, `8000` ms cumulative and `8` wait calls per
  `CuHost`. Zero, negative, non-integer, over-bound and over-budget requests
  are named refusals before any sleep. No busy-wait, no retry loop, no hidden
  polling.
- The snapshot is taken **after** the sleep and creates a new generation.
  `as_of` pinning, stale-generation refusal and every earlier observe arm are
  unchanged.
- `disclose` publishes the remaining wait budget as additive evidence; no new
  capability row, no new `CuVerdict`, no ABI bump, no new crate, no new OS
  grant, no native surface change and no G boundary change.
- Every 0.0.6-0.0.12 reader assertion stays. The new reader rule recomputes
  elapsed >= requested, cumulative sums, call counts, generation increase and
  every ceiling refusal, and rejects a missing or mismatched `wait` field.

## Work breakdown and gates

```text
0.0.13 — bounded observation wait
├── [v] W1 — the bounded wait contract
│   files: src/cu.rs, src/wire.rs
│   invariant: cu-abi 1.0, 12-row matrix unchanged, four cfgs type-check,
│   cargo test --locked green, no new capability row, no new grant,
│   no native surface change
├── [v] W2 — a real wait journey in CI
│   files: examples/cu_journey.rs, scripts/cu-journey-verify.py,
│          .github/workflows/cu-journeys.yml
│   invariant: required observe_wait_after_act + over-bound refusal on all
│   three OSes; reader recomputes wait evidence and generation freshness
├── [v] W3 — the truth of the docs
│   files: PRD.md, README.md, PRD_01_08, PRD_01_12, PRD_01_16,
│   PRD_01_17, this plan, release notes
│   invariant: B7b / D4 marked delivered; A5/B8/AT-SPI/AX/F3/C5/C6 and
│   the model loop stay deferred; the lifted [no] is recorded as an
│   owner-delegated decision; the W0 learning loop is recorded
└── [v] W4 — version bump and public signed Release
    files: Cargo.toml, Cargo.lock, release-policy.json,
           loader/ape-version.rc, .github/release-notes.md
    invariant: four version sources in ONE commit; dry-run before publish;
    the four release invariants must not regress
```

The controller owns W1/W2 implementation, W3/W4 planning, the three-OS CI
gate, the independent journey recompute, the release gate and the owner-view
acceptance. Under the owner's 2026-11-06 direction, implementation is direct;
the CU wire orchestration driver remains an L1 experiment, not a release
gate. No temporary `.py` patch script is used; the repository's independent
journey reader remains the CI authority.

## Delivery record

W1/W2 `40276ae`; W3/W4 `59e6fd2`. Three-OS CI `42213269581` /
`42213269486` (W1/W2) and `42214274457` / `42214326088` (W3/W4) success.
The independent reader recomputed the released journeys PASS on Linux 32 /
macOS 28 / Windows 35 performed steps, including `observe_wait_after_act` and
the `observe_wait_over_bound` refusal. `v0.0.13` was published at
2026-11-06T12:17:48Z from pack `42214546620`, candidate `42216049183`,
signing `42216112607`, dry-run `42216372517`, publish `42216487467`.
Owner-view acceptance: public page and assets anonymous 200, 15 assets, six
recomputed sidecars, build/signing receipts matched `59e6fd2` / `0.0.13`,
three signed assets matched `after_sha256` and were locally `Valid` with
RFC 3161 timestamps, and the downloaded APE printed
`minicon-harness 0.0.13`.

## Risks and downgrade rules

- **A wait is not a loop.** One bounded sleep per call; no polling until a
  condition becomes true, no model turn, no cost accounting beyond the three
  D4 integers.
- **Monotonic time, never wall-clock arithmetic.** `elapsed_ms` is measured
  with a monotonic clock and is evidence, not a request echo.
- **No reader assertion is weakened.** If a cell cannot honour a wait it
  publishes a named refusal; a `BOUNDARY` note excuses nothing.
- **Scope creep**: no image arm, no new verb, no capability row, no ABI bump,
  no policy engine, no OS grant, no new crate, no native surface change. If
  W1/W2 needs one, split it into a separate PRD.

## Non-goals

- `[no]` model-visible image arm, batched actions, AT-SPI / AX a11y trees,
  app-level effect evidence, safety policy, a second model loop or an
  app-level condition wait in this version.
- `[no]` window-manager product behaviour; `minicon-winmgr` owns it.
- `[no]` requesting or changing an OS grant, display mode or DPI setting.
- `[no]` widening `screen` into host control; the G boundary in `PRD_01_08`
  is unchanged.
- `[no]` re-publishing or overwriting any shipped v0.0.4-v0.0.12 asset.
