# PRD 01.07 — actuation receipt and interaction closure

Status: `[v]` implemented, qualified on three OSes and shipped in the public
signed `v0.0.6` (2026-11-05). This PRD is the contract half of
`plan/plan-0.0.6-interaction-closure.md`; the plan owns the work breakdown
and the CI gates, this document owns the observable contract. Evidence:
compile gate **42056894173**, journeys **42056894326**, publish **42060339772**.

## Problem

`cu-abi 1.0` has six verdicts and a strict readback rule, but a successful
click currently proves only that the **pointer** ended up where the caller
asked. `src/cu.rs:923-927` already turns `press=true` into a real
`Act::Click`, and `src/wire.rs:343-351` already advertises `button=middle` and
`double=true`; neither has ever been driven in a journey. A pointer readback
cannot distinguish "a button event was posted" from "the pointer was moved to
the right place and nothing was clicked". That is an honesty gap in the
evidence, not a missing feature.

## Scope

Close the gap **without** extending the capability matrix, the wire inputs or
the ABI major. Windows stays 12/12 declared verbs, Linux 10/12, macOS 8/12,
other 0/12. The six cells that are `false` remain named refusals; their unlock
conditions are recorded in W3 of the plan, not quietly reclassified here.

## Contract

### Actuation receipt

Every `act` that dispatches a button event returns, in addition to the
existing `readback` text, a typed receipt written by `CuHost` into
`evidence["actuation"]`:

| Field | Type | Rule |
|-------|------|------|
| `button` | string | `"left"` / `"middle"` / `"right"`; exactly the requested normalised button |
| `downs` | integer | 1 for a single click, 2 when `double=true` |
| `ups` | integer | must equal `downs` |
| `double` | boolean | must equal the request's `double` |
| `target` | string | the resolved window id/title the event was posted to; non-empty |

The receipt is **additive**:

- `cu-abi 1.0` is unchanged. No ABI major bump, no new `CuVerdict`, no new
  capability name, no new wire input parameter.
- The model-visible `readback` string keeps its current forms
  (`pointer at X,Y`, `scrolled N at X,Y`), so `pointer_from_readback()` and
  the existing text assertions keep working.
- `disclose` continues to publish the 12-row matrix. The receipt is evidence
  of a single call, not a declared capability.

### What the receipt asserts -- and what it does not

It asserts that the backend dispatched a real button event, with the requested
button and count, at the resolved coordinates, and that the pointer readback
landed in the target rect where a rect exists.

It does **not** assert that the target application's business action ran. No
UIA/AX/AT-SPI state, no file change, no navigation and no application-level
side effect is claimed. An app-level echo-window fixture would be required for
that, and is an explicit non-goal of 0.0.6. Writing "the click worked" in a
PRD without that distinction is the failure mode this contract exists to
prevent.

### Verdict rules

| Situation | Verdict |
|-----------|---------|
| event dispatched, receipt complete and matching, readback in rect | `Performed` |
| receipt missing on a path that declares support | `Unknown` (unconfirmed outcome) |
| receipt `button` / `downs` / `ups` / `double` disagrees with the request | `NotPerformed` |
| readback outside the control rect | `NotPerformed` (existing rule) |
| missing library / `$DISPLAY` / OS grant | `Unsupported`, named `cu: unsupported:` |
| no capability at all on this cell | `Refused`, with the matrix in evidence |
| `as_of` pinned to an older generation | `Stale` |

A `performed` step with no receipt is never upgraded by the independent
reader. A missing OS grant is never converted into an empty success.

### Follow-up observation

A journey is not complete after the last act. The final `observe` must have a
strictly greater `generation` than the first, and must still report at least
one window and one display. This proves the session survived the side-effecting
steps rather than the tool returning cached state.

## Verification

- `src/cu.rs`: unit tests for receipt completeness, mismatch -> `NotPerformed`,
  missing receipt -> `Unknown`, and the existing stale/admission matrix.
- `scripts/cu-cfg-check.py`: whole-file `rustc` type-check for the windows,
  macOS, Linux and fallback configurations -- all four backends must expose
  the same function signatures.
- `.github/workflows/cu-platforms.yml`: three OSes, `cargo test --locked`,
  release link, matrix test unchanged at 12/10/8.
- `.github/workflows/cu-journeys.yml`: required steps for `middle`, `right`,
  `double`, middle-drag and (Windows) `act:control` with `press=true`, on
  side-effect-free fixtures. `scripts/cu-journey-verify.py` independently
  recomputes the matrix, pointer readbacks, **receipts**, PNG IHDR and
  generation order from `cu-journey.json`.
- A `BOUNDARY` note (named OS-grant refusal on macOS) is still not `[v]`, and
  must not be usable to excuse a missing receipt on a `performed` step.

## Non-goals

- `[no]` macOS AX tree, Linux AT-SPI tree, Windows UIA COM tree. Unlock
  conditions: a runner with an Accessibility grant, or an AT-SPI D-Bus bus
  plus an ATK-enabled fixture, each proven before a capability is declared.
- `[no]` app-level state change evidence; no self-hosted echo window in 0.0.6.
- `[no]` Wayland-native protocol, image arm, runtime backend selection.
- `[no]` new crate, link-time platform library, runtime, plugin host,
  bun/WASM/ujs embedding. PRD 31 readiness stays BLOCKED at an immutable
  `unisacc`/`ujs` revision.
- `[no]` weakening any reader assertion to make a journey green. If a variant
  cannot be qualified on an OS, it is a named refusal with the OS and reason
  in the journey record, and the matrix does not change to hide it.

## Where this goes next

Only after the interaction closure is green on three OSes does it make sense
to discuss an echo-window fixture (application-level effect evidence) or the
accessibility trees. Both need a separate PRD, a separate CI fixture and a
separate gate. The dynamic-logic module envelope remains gated on the PRD 31
readiness items in `prd/PRD_01_04_cu_base.md:118-122`.
