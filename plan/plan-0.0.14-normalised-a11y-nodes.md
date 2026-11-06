# 0.0.14 — normalised a11y node schema (public signed Release accepted)

Status: `[v]` W1-W4 delivered in `31e257d` (W1/W2) and `efe96a2` (W3/W4),
after `v0.0.13` was published as a public signed Release and accepted from
the owner's view. Three-OS CI `42217541734` / `42217725863` and
`42218173775` / `42218235696` are success; `v0.0.14` is a published,
accepted signed Release. The source tree stayed at `0.0.13` until W3.

Baseline: `v0.0.13` closed B7b / D4 with a bounded observation wait. `v0.0.11`
shipped the read-only Windows MSAA tree, `v0.0.12` handle-addressed focus.
`cu-abi 1.0`, the 12/10/8/0 matrix and the four-backend type-check are
unchanged.

## Decision

W0 (`prd/PRD_01_17_frontier_radar.md`, 2026-11-06) found that a cross-platform
CU agent needs **one normalised node schema** — role, name, state, value,
bounds, relations — with per-platform adapters, and that UIA is the successor
to MSAA on Windows. The 0.0.11 tree published only role / name / bounds /
depth. This version closes the schema gap inside the existing MSAA arm:

- each `a11y_controls[]` row gains `value` (`accValue` BSTR), the raw `state`
  `STATE_SYSTEM_*` bit mask (`get_accState`) and `state_names` derived from a
  fixed, bounded bit table;
- the frame publishes `a11y_node_schema: "role-name-state-value-bounds-depth"`;
- the independent reader recomputes `state_names` **from the published raw
  integer** and rejects a guessed set, a missing value or a wrong schema.

This is the deterministic normalisation layer that UIA / AT-SPI / AX adapters
would translate into later. It is additive evidence on the existing `observe`
capability: no new verb, no new capability row, no ABI bump, no new OS grant,
no new crate, no native signature change and no G boundary change.

| Deferred, with the reason recorded | Unlock condition |
|---|---|
| UIA-first mixed routing (UIA tree + MSAA fallback) | hand-written `IUIAutomation` COM vtable verification on a real Windows desktop; a wrong slot is a crash, not a refusal |
| Incremental a11y change subscription | three-OS AT-SPI / AX grants + fixtures; the schema above is its prerequisite |
| A5 / D2 model-visible image arm | PNG budget / retention decision |
| AT-SPI (Linux) / AX (macOS) trees | OS grants + three-OS fixtures |
| F3 app-level effect evidence, C5 / C6 safety | side-effect-free fixture + policy PRD |
| Resource-domain telemetry or control | separate PRD + explicit owner decision on G |

## Work breakdown and gates

```text
0.0.14 — normalised a11y node schema
├── [v] W1 — the schema contract
│   files: src/screen.rs, src/cu.rs, src/wire.rs
│   invariant: cu-abi 1.0, 11 capability rows unchanged, four cfgs
│   type-check, cargo test --locked green, no new grant or crate
├── [v] W2 — an independently recomputed schema journey
│   files: scripts/cu-journey-verify.py,
│          .github/workflows/cu-journeys.yml
│   invariant: every a11y row's state_names equals the reader's own bit-mask
│   derivation; value is a string; the frame names the schema; no earlier
│   0.0.6-0.0.13 assertion is weakened
├── [v] W3 — the truth of the docs
│   files: PRD.md, README.md, PRD_01_08, PRD_01_17, PRD_01_18,
│   this plan, release notes
└── [v] W4 — version bump and public signed Release
    files: Cargo.toml, Cargo.lock, release-policy.json,
           loader/ape-version.rc, .github/release-notes.md
    invariant: four version sources in ONE commit; dry-run before publish
```

The controller owns W1/W2 implementation, W3/W4 planning, the three-OS CI
gate, the independent journey recompute, the release gate and owner-view
acceptance. No temporary `.py` patch script is used.

## Risks and downgrade rules

- **A wrong COM slot is a crash.** `accValue` (slot 11) and `get_accState`
  (slot 14) reuse the already-proven `IAccessible` vtable layout from 0.0.11;
  the Win64 24-byte `VARIANT` layout is unchanged. A failed call publishes an
  empty value / zero mask, never a guess.
- **State names are derived, never free-form.** Unknown bits stay in the raw
  integer. The reader recomputes the whole list from the integer.
- **No reader assertion is weakened.** A cell without the platform a11y API
  still publishes an empty tree plus a named reason.

## Non-goals

- `[no]` UIA / UIA3 Raw routing, incremental a11y subscriptions, an image arm,
  batched actions, app-level effect evidence, safety policy or a model loop in
  this version.
- `[no]` window-manager product behaviour; `minicon-winmgr` owns it.
- `[no]` requesting or changing an OS grant, display mode or DPI setting.
- `[no]` widening `screen` into host control; the G boundary in `PRD_01_08`
  is unchanged.
- `[no]` re-publishing or overwriting any shipped v0.0.4-v0.0.13 asset.
