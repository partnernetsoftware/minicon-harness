# PRD 01.16 — bounded observation wait (0.0.13)

Status: `[v]` W1/W2 delivered in `40276ae`, W3/W4 in the release commit,
after `v0.0.12` was published as a public signed Release and accepted from
the owner's view on 2026-11-06. Three-OS CI `42213269581` / `42213269486`
are success. Owner of the **contract** for 0.0.13;
`plan/plan-0.0.13-bounded-wait.md` owns the work breakdown, gates and release
chain. Nothing here claims a capability that a CI run has not produced.

Legend: `[v]` delivered with evidence, `[_]` planned, `[no]` explicit
non-goal.

## Domain fit

Inside KVM + W + S (`PRD_01_08`) this is **B7b / D4**: observation freshness
and a bounded budget around a desktop read. It is not a window manager, grants
no resource authority, posts no new kind of input, does not enumerate
processes for management and never borrows the `PRD_01_07` button actuation
receipt. The G resource-domain boundary is unchanged.

`PRD_01_12` defines L2 as the lowest level at which a self-sustained
`observe -> decide -> act -> wait -> re-observe` loop can close. This PRD
supplies only the bounded `wait` primitive and its D4 ceilings. It does not
supply A5 / D2, a model loop, app-level condition waiting or L2 itself.

## Problem

The L1 self-hosted orchestration trial produced two hard failures. An
`observe` issued immediately after an `act` captured the pre-reflow Chromium
frame, and `type` plus its send click had to be split into separate runs
before the new state was visible. No current wire field lets a caller ask for
a bounded delay before a fresh observation, and no evidence reports how long
the host actually waited or how much of the wait budget remains. A caller
therefore guesses with repeated observations, which is unbounded, unauditable
and still not evidence produced by the CU contract.

A coordinate basis (0.0.10), a bounded Windows MSAA tree (0.0.11) and a
handle-addressed act target (0.0.12) fix identity and location, not timing.

## Contract

### Additive parameter and evidence

```text
observe { op:"observe", wait_ms: 250 }
  -> evidence["wait"] = {
       "requested_ms": 250, "elapsed_ms": 251,
       "cumulative_ms": 251, "wait_calls": 1,
       "generation": <fresh observe generation>
     }
observe { op:"observe" }
  -> unchanged behaviour; no wait evidence
```

- `wait_ms` is an integer `1..=2000`. It sleeps once before the snapshot;
  the snapshot is taken after the sleep and creates a new generation.
- `evidence["wait"]` is published only for a performed wait. `elapsed_ms`
  comes from a monotonic clock; `cumulative_ms` and `wait_calls` come from
  `CuHost` state, never from the request.
- `disclose` publishes the remaining wait budget (`cumulative_ms`,
  `wait_calls`, `max_cumulative_ms`, `max_wait_calls`) as additive evidence.
  No new capability row, no new `CuVerdict`, no ABI bump: `cu-abi 1.0`,
  `CAPABILITIES` and the 12/10/8/0 matrix are unchanged.
- No native surface change: the four backends and
  `scripts/cu-cfg-check.py` keep their existing signatures.

### Bounds (D4)

| Bound | Value | Refusal |
|---|---|---|
| per-call wait | `1..=2000` ms | named `refused` before any sleep |
| cumulative wait per `CuHost` | `8000` ms | named `refused` before any sleep |
| wait calls per `CuHost` | `8` | named `refused` before any sleep |

Zero, negative, non-integer, over-bound and over-budget requests are named
refusals. There is no busy-wait, no retry loop, no hidden polling and no
condition wait.

### Admission and honesty

1. `observe` stays a read; `--allow-screen` is not required for the wait
   itself, and every existing observe admission rule is unchanged.
2. Ceiling checks happen before the sleep. A refused wait posts no delay and
   publishes no partial `wait` evidence.
3. A performed wait must publish a positive integer `elapsed_ms` that is at
   least `requested_ms` and at most `requested_ms + 500`; the independent
   reader recomputes it. A missing or mismatched field is `not_performed`.
4. `cumulative_ms` must equal the sum of all earlier `elapsed_ms` values in
   the same journey, and `wait_calls` must increment by one; a contradiction
   is `not_performed`.
5. The wait snapshot must have a generation greater than the immediately
   preceding observation. `as_of` pinning and stale-generation refusal are
   unchanged.
6. `CuHost::drop`, held-key state, focus state and every other effect are
   untouched. Wait state is three integers, not a loop.

## Verdict rules

| Situation | Verdict |
|---|---|
| bounded wait admitted, fresh snapshot, evidence recomputes | `performed` |
| `wait_ms` junk, over bound, over cumulative or over call count | `refused` (named) |
| wait evidence missing, elapsed too short/long or cumulative mismatch | `not_performed` (reader) |
| wait snapshot generation did not increase | `not_performed` (reader) |
| ordinary `observe` without `wait_ms` | unchanged `performed` |
| `--allow-screen` was never granted | unchanged; wait does not require an effect grant |

## Verification

- `src/cu.rs`: unit tests for parsing, all three ceiling refusals, the
  monotonic elapsed field, cumulative accounting, generation freshness, the
  `disclose` budget rows and the unchanged capability row / ABI. All
  0.0.6-0.0.12 tests stay.
- `scripts/cu-cfg-check.py`: all four backend configurations still type-check;
  no new native function is introduced.
- Three-OS CI journeys add a required `observe_wait_after_act` step plus an
  over-bound refusal. The independent reader recomputes `wait` evidence,
  cumulative sums, call counts, generation increase and every earlier
  assertion. No reader assertion is weakened to make a journey green.

## Non-goals

- `[no]` model-visible image arm, batched actions, AT-SPI / AX a11y trees,
  app-level effect evidence, safety policy, a second model loop or an
  app-level condition wait.
- `[no]` a 13th capability row, ABI bump or any change to the six false cells.
- `[no]` window-manager product behaviour; `minicon-winmgr` owns it.
- `[no]` requesting or changing an OS grant, display mode or DPI setting.
- `[no]` new crate, runtime, engine, plugin host, bun/WASM/ujs embedding.
- `[no]` claiming L2 is reached before A5 / D2, B7b + D4 and A4 / equivalent
  all have their own three-OS evidence.
