# 0.0.11 — structured control disclosure

Status: `[v]` W1/W2 delivered in commit `f0433d6`; W3/W4 run after the
three-OS CI gate. **The source tree stays at `0.0.10` until W4 runs.**

Baseline: `v0.0.10` is a public signed Release and was accepted from the
owner's view. `cu-abi 1.0`, the 12/10/8/0 matrix and the four-backend
type-check are unchanged.

## Decision

Re-plan 0.0.11 from "handle-addressed window targeting" to **structured
control disclosure**: give `observe` an `a11y` arm that returns a bounded,
read-only accessibility tree with names, roles and virtual-desktop rectangles.
The reason is measured, not guessed: the L1 self-hosted orchestration trial
could not read a Chromium window's input box or send button through
`controls[]` (one `Chrome_RenderWidgetHostHWND`, `role=unknown`) and fell back
to screenshots plus a vision read of pixel coordinates. The owner required on
2026-11-06 that the missing capability be built into this version.

This is an owner decision that lifts one earlier `[no]` ("a11y trees"), only
for the Windows MSAA read-only subset declared in `PRD_01_15`. The original
handle-addressed `act:focus` contract (`PRD_01_14`) is **moved to 0.0.12**,
not cancelled: the `a11y` hwnd parameter is a scan target, not an act target.
The two WorkBuddy windows sharing the title `WorkBuddy` remain the measured
motivation for that follow-up.

| Deferred, with the reason recorded | Unlock condition |
|---|---|
| `act:focus` by `hwnd` + `target_hwnd` | 0.0.12, `PRD_01_14` |
| AT-SPI (Linux) / AX (macOS) trees | OS grants + three-OS fixtures; `unknown` with a reason here |
| A5 / D2 model-visible image arm | separate PRD + PNG budget/retention decision |
| B7b `wait` + D4 ceilings | loop/budget decision; hard evidence: pre-reflow `observe` frames and `type`/send needing separate runs |
| F3 app-level effect evidence | side-effect-free echo fixture on three OSes |
| C5 / C6 safety + hand-off, B8 batched actions | policy PRD / admission+stale design |

## Work breakdown and gates

```text
0.0.11 — structured control disclosure
├── [v] W1 — the a11y contract and the Windows tree
│   files: src/screen.rs, src/screen_linux.rs, src/screen_macos.rs,
│          src/cu.rs, src/lib.rs, src/wire.rs
│   invariant: cu-abi 1.0, 12-row matrix unchanged, four cfgs type-check,
│   109 unit tests green, no new capability row, no new crate or grant
├── [v] W2 — a required CI step and an independent recompute
│   files: examples/cu_journey.rs, scripts/cu-journey-verify.py,
│          .github/workflows/cu-journeys.yml
│   invariant: observe_a11y required on all three OSes; cells without an
│   a11y API publish empty tree + named reason; reader recomputes every row
├── [_] W3 — the truth of the docs
│   files: PRD.md, README.md, PRD_01_08, PRD_01_12, PRD_01_14, PRD_01_15,
│   this plan, release notes
│   invariant: the lifted [no] is recorded as an owner decision; every other
│   [no] and the G boundary are unchanged
└── [_] W4 — version bump and public signed Release
    files: Cargo.toml, Cargo.lock, release-policy.json,
           loader/ape-version.rc, .github/release-notes.md
    invariant: four version sources in ONE commit; dry-run before publish;
    the four release invariants must not regress
```

## Delivery record

Filled in after the public signed Release is accepted from the owner's view:
commit SHA, pack / candidate / signing / dry-run / publish run ids, anonymous
page and asset status, sidecar recompute, signing-cell hashes and the version
the downloaded APE prints.

## Risks and downgrade rules

- **MSAA is not UIA.** Some self-drawn controls expose only coarse roles or
  empty names; `role_name=unknown` is honest, never guessed.
- **Handles are not stable identities.** Every scan re-validates the handle
  against a fresh live window list; a stale handle is a named reason.
- **No reader assertion is weakened.** If a CI cell cannot produce a tree it
  publishes an empty list plus a reason, and the reader accepts that shape.
- **Scope creep**: no image arm, no `wait`, no new crate, no ABI bump, no
  policy engine, no OS grant request.

## Non-goals

- `[no]` image arm, explicit `wait`, batched actions, app-level effect
  evidence and safety policy in this version.
- `[no]` window-manager product behaviour; that belongs to `minicon-winmgr`.
- `[no]` requesting or changing an OS grant, display mode or DPI setting.
- `[no]` widening `screen` into host control; `PRD_01_08`'s G boundary stands.
- `[no]` re-publishing or overwriting any shipped v0.0.4-v0.0.10 asset.
