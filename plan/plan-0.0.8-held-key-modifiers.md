# 0.0.8 — held-key modifiers: a keyboard state that can never be left stuck

Status: `[_]` plan + contract delivered after `v0.0.7` was published, as the
owner's "plan the next version after each public release" loop requires.
**No source, version or Release change.** The version still in the source
tree is `0.0.7` until W4 of this plan runs.

Baseline: `v0.0.7` is a public company-signed Latest Release. Both halves of
B6/B7 that are pure desktop verbs (path drag, two-axis scroll) are qualified
on three OSes; the capability matrix is windows 12/12, linux 10/12, macos
8/12, other 0/12, and `cu-abi 1.0` is frozen. Compiled `screen` backends are
type-checked in four configurations.

## Decision: what 0.0.8 is, and what it deliberately is not

The owner's direction for this loop is unchanged: **concentrate on computer
use, do not grow harness capabilities**. Of the deferred B6/B7/B8 halves, only
one is a desktop verb with no loop decision and no grant change:

| Delivered in 0.0.8 | Leaf | Shape |
|---|---|---|
| held-key modifiers | B6b | `act:key` gains bounded `hold` / `release` parameters; a modifier stays down across calls, is disclosed, and is released on a foreground change or at host teardown |

| Deferred, with the reason recorded so it is not re-litigated | Unlock condition |
|---|---|
| explicit bounded `wait` (B7b) | a loop/budget decision (D4), **not** a desktop verb. It would add a 13th capability row and change every reader's `EXPECTED`; the owner explicitly deferred harness-side capability work |
| batched `actions[]` (B8) | one call with several effects conflicts with "one act -> one verdict -> one receipt" and with `as_of` pinning; needs its own admission/stale design |
| a11y trees, image arm, safety policy, environment/grant disclosure, app-level effect evidence (0.7.1, 0.7.2, 0.7.4, 0.7.5, 0.7.6) | unchanged unlock conditions in `PRD.md`; none is met by a hosted runner today |

Consequence: **the capability matrix still does not grow**, for the third
consecutive version. `hold` / `release` are additive parameters on the
existing `act:key` capability, not a new capability name. `cu-abi` stays 1.0.
This is the same defensible boundary as 0.0.6 and 0.0.7: strengthen the
expressiveness and evidence of declared verbs before declaring new ones.

## Contract owner: `prd/PRD_01_10_held_key_modifiers.md`

- `act:key`: `keys` stays the single combination spelling (`ctrl+s`). Optional
  `hold: true` leaves **only whitelisted modifier keys** down after the call;
  optional `release: true` releases everything held. `hold` and `release` in
  one call is a named refusal.
- The held set is owned by `CuHost`, not by a native global: at most four
  modifiers, disclosed in `evidence["held_keys"]` of `op=disclose`, and
  published as `evidence["keyboard"]` on every `act:key`. It is **not** a
  button actuation receipt; blurring the two would make `PRD_01_07` assert
  something it never measured.
- A foreground change between calls, a lost grant, or host teardown releases
  everything held **before** any further effect and reports the release, never
  silently. A stuck modifier on a shared runner is a correctness bug, not a
  cosmetic one.
- One function set, four configurations: `native::set_modifier` /
  `native::release_all_modifiers` change together on Windows, macOS, Linux
  and the fallback backend; `scripts/cu-cfg-check.py` type-checks all four.
- Pointer, drag, scroll, focus and move semantics from 0.0.6/0.0.7 are
  untouched. Every 0.0.6/0.0.7 reader assertion stays.

## Work breakdown and gates

```text
0.0.8 — held-key modifiers
├── [_] W1 — the keyboard state machine in the contract
│   files: src/cu.rs, src/wire.rs, src/screen.rs, src/screen_macos.rs,
│          src/screen_linux.rs, scripts/cu-cfg-check.py
│   invariant: cu-abi 1.0, 12-row matrix unchanged, four cfgs type-check,
│   cargo test --locked green on all three CI jobs
│   evidence: cu-platforms.yml run, matrix test still 12/10/8/0
├── [_] W2 — real held-key journeys in CI
│   files: examples/cu_journey.rs, scripts/cu-journey-verify.py,
│          .github/workflows/cu-journeys.yml
│   invariant: required steps act_key_hold, disclose_held,
│   act_key_release, disclose_released on all three OSes; foreground-loss
│   auto-release is unit-tested; the journey always ends with an empty set
│   evidence: cu-journeys.yml run, three cu-journey.json artifacts,
│   independent reader recomputing the held set and its release
├── [_] W3 — the truth of the docs
│   files: PRD.md, README.md, prd/PRD_01_03_computer_use.md,
│   prd/PRD_01_08_computer_use_scope.md, prd/PRD_01_10_held_key_modifiers.md,
│   release notes
│   invariant: B6b marked delivered, B7b/B8 still deferred with unlock
│   conditions, no stale "planned" claim for a shipped verb
└── [_] W4 — 0.0.8 version bump and public signed Release
    files: Cargo.toml, Cargo.lock, release-policy.json,
           loader/ape-version.rc, .github/release-notes.md
    invariant: four version sources in ONE commit; dry-run before publish;
    the four release invariants must not regress
    evidence: run IDs + owner-view anonymous download and re-hash
```

### W2 step design

| New step | Request | Required on | Why it is harmless |
|---|---|---|---|
| `act_key_hold` | `action=key, keys="ctrl", hold=true` | all 3 | one modifier down in an empty client area; no shortcut is delivered yet |
| `disclose_held` | `op=disclose` | all 3 | reads the held set back from the host, no new effect |
| `act_key_release` | `action=key, keys="ctrl", release=true` | all 3 | releases everything before any further click |
| `disclose_released` | `op=disclose` | all 3 | proves the session did not end with a stuck modifier |

The independent reader is extended, not weakened:

- `EXPECTED` stays exactly 12 rows with windows 12 / linux 10 / macos 8 true.
- `act_key_hold` publishes `evidence["keyboard"]["held"]` containing exactly
  the requested modifier; `disclose_held` reports the same set.
- `act_key_release` publishes a non-empty `released` list; both
  `disclose_released` and the final `op=disclose` report an empty held set.
- A performed `act:key` never gains a button actuation receipt; the reader
  asserts `evidence["actuation"]` is null for these steps.
- Every existing 0.0.6/0.0.7 assertion (receipts, `path_points`, both scroll
  axes, generations, live final frame, declared-but-skipped rule, BOUNDARY)
  stays. A `BOUNDARY` note must never excuse a stuck modifier.

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
`minicon-harness 0.0.8`.

## Risks and downgrade rules

- **A stuck modifier is the primary risk.** If the foreground changes, the
  grant is lost, or the host is dropped, all held keys are released before
  any other effect. `disclose` must report the release, not hide it.
- **macOS flags**: `CGEventSetFlags` must be applied to every event while a
  modifier is held, not only to the down event; otherwise the synthetic
  keyboard state and the CG flags disagree.
- **Linux keycodes**: XTest uses X11 keysyms resolved at runtime; an unmapped
  modifier is a named `unsupported`, never a guessed keycode.
- **Win32**: `keybd_event` down/up pairs must be symmetric; a `release` after
  a session that already lost its foreground must still be safe to call.
- **No assertion is relaxed.** If a variant cannot be qualified on an OS it is
  a named refusal with OS and reason in the journey record; the matrix does
  not change to hide it, and the reader does not drop the check.
- **Scope creep**: no new crate, link-time platform library, tool, ABI bump,
  engine, image arm or policy engine. If W1/W2 needs one, split it into a
  separate PRD rather than widening 0.0.8.

## Non-goals

- `[no]` a11y trees (AX / AT-SPI / UIA COM), image arm in the wire codec,
  Wayland-native protocol, runtime backend selection.
- `[no]` explicit bounded `wait`, batched `actions[]` in this version; each
  has an unlock condition above.
- `[no]` app-level state-change evidence; a self-hosted echo window is a
  separate design and gate.
- `[no]` new crate, runtime, plugin host, bun/WASM/ujs embedding; PRD 31
  readiness stays BLOCKED.
- `[no]` re-publishing or overwriting v0.0.4 / v0.0.5 / v0.0.6 / v0.0.7
  assets.
- `[no]` claiming `[v]` for a verb that only compiled, or relaxing a reader
  assertion to make a journey green.

## Delivery record

Filled in as each W closes, with the CI run IDs, the three journey artifacts,
the release chain runs and the owner-view anonymous acceptance result. Until
then every W above stays `[_]` and the source version stays `0.0.7`.
