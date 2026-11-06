# 0.0.11 — handle-addressed window targeting

Status: `[_]` planned after `v0.0.10` was published as a public signed Release
and accepted from the owner's view. **No source, version or Release change
belongs to this document.** The source tree stays at `0.0.10` until W4 runs.

Baseline: `v0.0.10` published the coordinate basis (`display_scales[]` in
`disclose`, `scale` in `observe.displays[]`) with three-OS CI evidence and
corrected the earlier ~1.25x rationale. `cu-abi 1.0`, the 12/10/8/0 matrix
and the four-backend type-check are unchanged. Five consecutive versions
(0.0.6-0.0.10) delivered desktop evidence without declaring a new capability
row.

## Decision

0.0.11 gives the W (window target binding) axis a **handle arm**: a caller can
name a window by the `hwnd` that `observe.windows[]` already publishes, and
the host reports the handle of the foreground window in its readback.

The reason is a measured failure in the L1 self-hosted orchestration trial,
not a guess. The controller drove two WorkBuddy client windows side by side;
both have the title `WorkBuddy`. `act:focus` resolves a window by unique
title, so the controller could not name the sub-session window it wanted and
fell back to clicking pixel coordinates to switch sessions. A coordinate
basis cannot fix an ambiguous *identity*: the same rect can belong to either
window. L2 needs stable window identity before it can manage several session
windows, so handle targeting precedes the image arm.

| Deferred, with the reason recorded | Unlock condition |
|---|---|
| A5 / D2 model-visible image arm | separate PRD + PNG budget/retention decision |
| B7b `wait` + D4 ceilings | loop/budget decision, not a desktop verb; would add a 13th capability row. Hard L1 evidence: `observe` immediately after an `act` captured a pre-reflow Chromium frame, and `type` plus its send click had to be split into separate runs |
| A4 accessibility trees | OS grants + three-OS fixtures. Hard L1 evidence: the new-session welcome page's input box could not take CU keyboard focus at all |
| F3 app-level effect evidence / held-key effect evidence | side-effect-free echo fixture on three OSes |
| C5 / C6 safety + hand-off, B8 batched actions | policy PRD / admission+stale design |

The capability matrix still does not grow: `hwnd` is an additive parameter and
`target_hwnd` is additive evidence, not a new capability name. `cu-abi` stays
1.0 and the six false cells are untouched.

## Contract owner: `prd/PRD_01_14_handle_addressed_targeting.md`

- `act:focus` gains an optional `hwnd` (integer, from a prior `observe`).
  `window` (title) and `hwnd` are mutually exclusive: giving both, or neither,
  is a named refusal before any native effect.
- The handle is re-validated on every call against the live window list
  (`handle_is_live`-equivalent) and against the title it reports; a stale or
  reused handle is `not_performed` or `stale`, never a silent best effort.
- `focus` readback and the actuation receipt gain `target_hwnd`: the handle of
  the foreground window at dispatch time. It is read from the desktop, never
  echoed from the request. With `window` (title) targeting the field is still
  published, so old callers gain identity evidence without changing their
  calls.
- No new OS grant, no window-manager behaviour, no process or memory
  authority: the G boundary in `PRD_01_08` is unchanged.
- One function set, four configurations: `native::window_by_handle()` changes
  together on Windows, macOS, Linux and fallback; `cu-cfg-check.py` types all
  four.
- Every 0.0.6-0.0.10 reader assertion stays. The new reader rule recomputes
  that `target_hwnd` equals the focused window in the same observation and
  rejects a missing or mismatched field.

## Work breakdown and gates

```text
0.0.11 — handle-addressed window targeting
├── [_] W1 — the handle-target contract
│   files: src/cu.rs, src/wire.rs, src/screen.rs, src/screen_linux.rs,
│          src/screen_macos.rs, scripts/cu-cfg-check.py
│   invariant: cu-abi 1.0, 12-row matrix unchanged, four cfgs type-check,
│   cargo test --locked green, no new capability row, no new grant
├── [_] W2 — a real handle-target journey in CI
│   files: examples/cu_journey.rs, scripts/cu-journey-verify.py,
│          .github/workflows/cu-journeys.yml
│   invariant: required focus_by_handle + stale-handle refusal on all three
│   OSes; reader recomputes target_hwnd against the focused window
├── [_] W3 — the truth of the docs
│   files: PRD.md, README.md, PRD_01_08, PRD_01_12, PRD_01_14,
│   this plan, release notes
│   invariant: E5b marked delivered, A5/B7b/A4/F3/C5/C6/B8 still deferred
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
decides. This is the L1 division that `PRD_01_12` records. The GUI dispatch
itself must use the CU wire driver (`examples/cu_orchestrate.rs`), never a
hand-written desktop automation script.

## Risks and downgrade rules

- **Handles are not stable identities.** A handle may be reused after a window
  closes; every call re-validates against the live list, and a mismatch is a
  named refusal, never a silent fallback to another window.
- **No reader assertion is weakened.** If a backend cannot resolve handles, it
  publishes `unknown` with a named reason in the journey record.
- **Scope creep**: no image arm, no a11y tree, no `wait`, no new crate, no
  link-time platform library, no ABI bump, no policy engine. If W1/W2 needs
  one, split it into a separate PRD.

## Non-goals

- `[no]` image arm, explicit `wait`, batched actions, a11y trees, app-level
  effect evidence and safety policy in this version.
- `[no]` window-manager product behaviour (minimise, tile, virtual desktops);
  that belongs to `minicon-winmgr`, not `screen`.
- `[no]` requesting or changing an OS grant, display mode or DPI setting.
- `[no]` widening `screen` into host control; the G boundary in `PRD_01_08`
  is unchanged.
- `[no]` re-publishing or overwriting any shipped v0.0.4-v0.0.10 asset.
